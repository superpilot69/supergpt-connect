import test from "node:test";
import assert from "node:assert/strict";
import { clients, clientFamily, modelSelectable } from "../src/clients.ts";

test("desktop and CLI choices are distinct, and Claude defaults to its desktop entry", () => {
  assert.deepEqual(clients.filter(c => clientFamily(c.id) === "claude").map(c => c.id), ["claude-desktop", "claude", "claude-vscode"]);
  assert.equal(clientFamily("codex-desktop"), "codex");
});
test("Claude Desktop cannot import an incompatible alias through bulk selection", () => {
  for (const id of ["claude-sonnet-5-5", "claude-opus-5-5", "anthropic/claude-haiku-test"]) assert.equal(modelSelectable("claude-desktop", id), true);
  for (const id of ["gpt-test", "custom-alias", "claude-sonnet-", "claude-sonnet-5[1m]"]) assert.equal(modelSelectable("claude-desktop", id), false);
  assert.equal(modelSelectable("claude", "custom-alias"), true);
  assert.equal(modelSelectable("codex-desktop", "custom-alias"), true);
});
