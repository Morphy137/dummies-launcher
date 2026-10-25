import { execFileSync, execSync } from "node:child_process";
import { mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const entries = new Map();
function licenseTexts(directory) {
  return readdirSync(directory, { withFileTypes: true })
    .filter(
      (file) =>
        file.isFile() && /^(licen[cs]e|copying|notice|copyright|authors)([._-]|$)/i.test(file.name),
    )
    .map((file) => `--- ${file.name} ---\n${readFileSync(join(directory, file.name), "utf8")}`);
}

const npm = JSON.parse(
  execSync("pnpm.cmd licenses list --prod --json", {
    cwd: root,
    encoding: "utf8",
    maxBuffer: 32 * 1024 * 1024,
  }),
);
for (const packages of Object.values(npm)) {
  for (const pkg of packages) {
    entries.set(`npm ${pkg.name} ${pkg.versions.join(", ")}`, {
      license: pkg.license,
      source: pkg.homepage ?? "",
      texts: pkg.paths.flatMap(licenseTexts),
    });
  }
}
for (const manifest of ["src-tauri/Cargo.toml", "crates/dummy-game/Cargo.toml"]) {
  const metadata = JSON.parse(
    execFileSync(
      "cargo",
      [
        "metadata",
        "--locked",
        "--format-version",
        "1",
        "--filter-platform",
        "x86_64-pc-windows-msvc",
        "--manifest-path",
        manifest,
      ],
      { cwd: root, encoding: "utf8", maxBuffer: 32 * 1024 * 1024 },
    ),
  );
  for (const pkg of metadata.packages) {
    const directory = dirname(pkg.manifest_path);
    const texts = licenseTexts(directory);
    if (pkg.license_file && !texts.length)
      texts.push(readFileSync(join(directory, pkg.license_file), "utf8"));
    entries.set(`cargo ${pkg.name} ${pkg.version}`, {
      license: pkg.license ?? "See license file",
      source: pkg.repository ?? "",
      texts,
    });
  }
}
const sections = [
  "Dummies dependency license inventory\nGenerated from installed production npm packages and both Cargo lockfiles.\nIncludes a conservative superset of build/platform dependencies. Review packages without bundled license text before distribution.\n",
];
let missing = 0;
for (const [name, entry] of [...entries].sort(([a], [b]) => a.localeCompare(b))) {
  if (!entry.texts.length) missing += 1;
  sections.push(
    `\n=== ${name} ===\nLicense: ${entry.license}\nSource: ${entry.source}\n${entry.texts.join("\n") || "No license text present in installed package; manual review required."}\n`,
  );
}
const destination = join(root, "src-tauri/resources/DEPENDENCY_LICENSES.txt");
mkdirSync(dirname(destination), { recursive: true });
writeFileSync(destination, sections.join("\n"));
console.log(
  `Generated ${entries.size} dependency notices (${missing} require manual text review).`,
);
