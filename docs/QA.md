# Manual release checklist

Use a disposable Windows profile/VM and a dedicated Steam test library. Never test deletion on real game installations.

- [ ] Windows 10/11; light/dark/system; forced colors; 100/150/200% DPI and two monitors.
- [ ] Keyboard-only search, grid arrows, list actions, detail Escape/focus restoration, dialogs.
- [ ] Settings Save, Cancel, Escape, backdrop; save denied; unusual imported durations/buffers.
- [ ] Catalog cached/offline, initial empty cache, failed refresh, changing query/page while loading.
- [ ] Launch 1/3 dummies; minimize each, restore through tray, close each; stop all; quit launcher.
- [ ] Launch shortcut with app closed/open/minimized; confirm single instance and managed timer.
- [ ] Crash launcher and confirm managed children terminate through EOF.
- [ ] Pause local timer; confirm Discord presence is intentionally unaffected; target/buffer/complete.
- [ ] Steam new dedicated folder; secondary library; refuse real folder/manifest and junctions.
- [ ] Modify tracked dummy/manifest externally; refuse destructive removal/update and retain both files.
- [ ] Add unrelated files to dummy folder; remove integration and confirm those files survive.
- [ ] Permissions denied during setup/remove/save; verify rollback and library identity.
- [ ] Signed installer upgrade from older installed version; stop sessions first; verify resources/CSP.
- [ ] Portable marker disables automatic install; replace ZIP manually, preserve AppData.
- [ ] Verify distributed notices/license texts for all bundled dependencies and artwork.
- [ ] Measure visible/hidden release process tree with 0/1/3 dummies; record OS, WebView2 and commit.

No real quest progress is inferred from the local timer. Verify current Discord/Steam rules and compatibility independently.
