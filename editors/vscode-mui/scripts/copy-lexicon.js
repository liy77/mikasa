// Bundles the shared Mikasa lexicon into out/ at build time.
//
// At runtime the extension first looks for ../mikasa-lexicon.json (works in the
// monorepo / dev host), then for out/mikasa-lexicon.json (the only copy present
// inside a packaged .vsix). This script produces that second copy so a packaged
// extension shows the exact same Mikasa keyword descriptions as mikasa-lsp —
// the single source of truth stays editors/mikasa-lexicon.json.

const fs = require("fs");
const path = require("path");

const src = path.join(__dirname, "..", "..", "mikasa-lexicon.json");
const outDir = path.join(__dirname, "..", "out");
const dest = path.join(outDir, "mikasa-lexicon.json");

try {
  fs.mkdirSync(outDir, { recursive: true });
  fs.copyFileSync(src, dest);
  console.log("bundled mikasa-lexicon.json -> out/mikasa-lexicon.json");
} catch (err) {
  console.error("failed to bundle mikasa-lexicon.json:", err.message);
  process.exit(1);
}
