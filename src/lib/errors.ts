import { invoke, isTauri } from "@tauri-apps/api/core";
import i18n from "../i18n";

// Keep technical backend diagnostics available in DevTools without exposing them as untranslated UI copy.
export function formatError(reason: unknown): string {
  const diagnostic = reason instanceof Error ? reason.message : String(reason);
  console.error("Dummies diagnostic:", diagnostic);
  if (isTauri()) void invoke("report_frontend_error", { message: diagnostic }).catch(() => {});
  const rules: Array<[RegExp, string]> = [
    [/ownership|verify ownership|left unchanged/i, "ownership"],
    [/stop (?:the|this|all)|not running/i, "stopFirst"],
    [/remove (?:the )?steam integration/i, "removeSteam"],
    [/already|duplicate|overwrite/i, "conflict"],
    [/restore.*bak|cannot read|corrupt|deserialize/i, "data"],
    [/invalid|numeric|outside|redirected/i, "path"],
    [/permission|denied|access/i, "permission"],
    [/not found|no longer exists|could not.*find/i, "missing"],
    [/network|http|fetch|timeout|connect|dns/i, "network"],
  ];
  const key = rules.find(([pattern]) => pattern.test(diagnostic))?.[1] ?? "generic";
  return i18n.t(`errors.${key}`);
}
