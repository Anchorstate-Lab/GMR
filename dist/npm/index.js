// Loads the same platform package bin/gmr.js runs, and hands back the addon.
const { createRequire } = require("node:module");

const scope = require("./package.json").name.split("/")[0];
const pkg = `${scope}/gmr-${process.platform}-${process.arch}`;

let addon;
try {
  addon = require(`${pkg}/gmr.node`);
} catch (cause) {
  throw new Error(
    `gmr: no prebuilt addon for ${process.platform}-${process.arch}. ` +
      `The CLI ships for the same platforms; if one is there and this is not, ` +
      `the platform package predates the addon.`,
    { cause },
  );
}

module.exports = { ...addon, CONTRACT: "gmr.contract.v12.0" };
