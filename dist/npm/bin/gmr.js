#!/usr/bin/env node
// Forwards to the platform package's binary. npm is a wrapper over the tarball,
// not the only door: dist/install.sh installs the same artifact without node.
//
// The probes live beside the binary inside the platform package, so this must
// exec the real path rather than copy the binary somewhere else — separating
// them leaves every `about:` unresolvable.
const { spawnSync } = require("node:child_process");

const pkg = `@gmr/cli-${process.platform}-${process.arch}`;

let binary;
try {
  binary = require.resolve(`${pkg}/bin/gmr`);
} catch {
  console.error(
    `gmr: no prebuilt binary for ${process.platform}-${process.arch}.\n` +
      `Install it directly instead:\n` +
      `  curl -fsSL https://raw.githubusercontent.com/Zongming-He/gmr/main/dist/install.sh | sh`,
  );
  process.exit(1);
}

const { status, error } = spawnSync(binary, process.argv.slice(2), {
  stdio: "inherit",
});
if (error) {
  console.error(`gmr: cannot run ${binary}: ${error.message}`);
  process.exit(1);
}
// observe and pass exit 1 when an anchor moved; that is a result, not a crash.
process.exit(status === null ? 1 : status);
