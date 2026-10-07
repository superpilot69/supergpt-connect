import test from "node:test";
import assert from "node:assert/strict";
import {
  DEFAULT_STATION,
  normalizeStationUrl,
  parseStationPreferences,
} from "../src/stations.ts";

test("new installs and corrupted preferences fall back to SuperGPT", () => {
  for (const raw of [null, "", "{", "{}", "null", '{"version":99}']) {
    assert.deepEqual(parseStationPreferences(raw), {
      version: 1,
      selected: DEFAULT_STATION.id,
      custom: [],
    });
  }
});
test("custom stations and selection survive serialization without persisting credentials", () => {
  const source = {
    version: 1,
    selected: "custom-test",
    apiKey: "sk-never-store",
    custom: [
      {
        id: "custom-test",
        name: " My gateway ",
        baseUrl: "https://api.example.test/prefix/v1/",
        apiKey: "sk-never-store",
      },
    ],
  };
  const loaded = parseStationPreferences(JSON.stringify(source));
  assert.deepEqual(loaded, {
    version: 1,
    selected: "custom-test",
    custom: [
      {
        id: "custom-test",
        name: "My gateway",
        baseUrl: "https://api.example.test/prefix",
      },
    ],
  });
  assert(!JSON.stringify(loaded).includes("sk-never-store"));
});
test("invalid destinations cannot be saved, including credentials in URLs", () => {
  for (const url of [
    "https://name:password@example.test",
    "https://example.test?key=secret",
    "https://example.test#token",
    "http://example.test",
    "file:///tmp/config",
    "https://example.test/v1/messages",
    "https://example.test/v1/responses",
    "https://example.test/v1/chat/completions",
    "example.test",
  ]) {
    assert.throws(() => normalizeStationUrl(url));
    assert.equal(
      parseStationPreferences(
        JSON.stringify({
          version: 1,
          selected: "custom-test",
          custom: [{ id: "custom-test", name: "Bad", baseUrl: url }],
        }),
      ).selected,
      "supergpt",
    );
  }
  assert.equal(
    normalizeStationUrl("http://127.0.0.1:18473/v1"),
    "http://127.0.0.1:18473",
  );
});
test("duplicate destinations, duplicate IDs, and attempts to replace the default are ignored", () => {
  const loaded = parseStationPreferences(
    JSON.stringify({
      version: 1,
      selected: "custom-removed",
      custom: [
        {
          id: "supergpt",
          name: "Imposter",
          baseUrl: "https://bad.example.test",
        },
        {
          id: "custom-copy",
          name: "Copy",
          baseUrl: "https://api.supergpt.dev/v1",
        },
        { id: "custom-one", name: "One", baseUrl: "https://one.example.test" },
        { id: "custom-one", name: "Two", baseUrl: "https://two.example.test" },
      ],
    }),
  );
  assert.equal(loaded.selected, "supergpt");
  assert.deepEqual(
    loaded.custom.map((item) => item.id),
    ["custom-one"],
  );
});
