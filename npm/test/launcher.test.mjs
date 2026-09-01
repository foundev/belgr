import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import test from "node:test";

import {
  installMethodEnvironment,
  isMainModule,
  nativeBinaryPath,
  launch,
  platformPackageName,
  resolveBundle,
} from "../launcher/belgr.js";

test("recognizes npm's symlinked bin entrypoint", () => {
  let resolved;
  assert.equal(
    isMainModule("/tmp/node_modules/.bin/belgr", (entrypoint) => {
      resolved = entrypoint;
      return new URL("../launcher/belgr.js", import.meta.url).pathname;
    }),
    true,
  );
  assert.equal(resolved, "/tmp/node_modules/.bin/belgr");
});

test("selects each published native package", () => {
  assert.equal(platformPackageName("darwin", "arm64"), "belgr-darwin-universal");
  assert.equal(platformPackageName("darwin", "x64"), "belgr-darwin-universal");
  assert.equal(platformPackageName("linux", "x64"), "belgr-linux-x64-gnu");
  assert.equal(platformPackageName("linux", "arm64"), "belgr-linux-arm64-gnu");
  assert.equal(platformPackageName("android", "arm64"), "belgr-android-arm64");
  assert.equal(platformPackageName("win32", "x64"), "belgr-win32-x64");
});

test("rejects unsupported platforms before attempting to run a binary", () => {
  assert.throws(() => platformPackageName("freebsd", "x64"), /does not publish/);
});

test("resolves the native package root from its manifest", () => {
  const bundle = resolveBundle(
    (name) => {
      assert.equal(name, "belgr-linux-x64-gnu/package.json");
      return "/tmp/node_modules/belgr-linux-x64-gnu/package.json";
    },
    "linux",
    "x64",
  );
  assert.equal(bundle, "/tmp/node_modules/belgr-linux-x64-gnu");
});

test("names the platform-native executable", () => {
  assert.equal(nativeBinaryPath("/tmp/bundle", "linux"), "/tmp/bundle/bin/belgr");
  assert.equal(nativeBinaryPath("C:\\bundle", "win32"), "C:\\bundle/bin/belgr.exe");
});

test("launches the native bundle with its siblings on PATH and npm ownership marked", () => {
  const child = new EventEmitter();
  child.kill = () => true;
  let invocation;
  launch("/tmp/bundle", ["--version"], "linux", (binary, args, options) => {
    invocation = { binary, args, options };
    return child;
  });
  assert.equal(invocation.binary, "/tmp/bundle/bin/belgr");
  assert.deepEqual(invocation.args, ["--version"]);
  assert.equal(invocation.options.stdio, "inherit");
  assert.equal(invocation.options.env.BELGR_MANAGED_BY_NPM, "true");
  assert.equal(invocation.options.env.BELGR_MANAGED_BY_NPX, undefined);
  assert.ok(invocation.options.env.PATH.startsWith(`/tmp/bundle/bin${process.platform === "win32" ? ";" : ":"}`));
});

test("distinguishes one-shot npx runs from npm installs", () => {
  assert.deepEqual(installMethodEnvironment({}), {
    BELGR_MANAGED_BY_NPM: "true",
  });
  assert.deepEqual(installMethodEnvironment({ npm_command: "exec" }), {
    BELGR_MANAGED_BY_NPX: "true",
  });
});

test("returns the conventional exit status when the native process is signalled", () => {
  const child = new EventEmitter();
  child.kill = () => true;
  let exitCode;
  launch(
    "/tmp/bundle",
    [],
    "linux",
    () => child,
    (code) => {
      exitCode = code;
    },
  );
  child.emit("exit", null, "SIGINT");
  assert.equal(exitCode, 130);
});
