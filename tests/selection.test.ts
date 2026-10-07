import { test } from "node:test";
import assert from "node:assert/strict";
import { EMPTY_SELECTION, selectModels } from "../src/selection.ts";

test("an empty selection stays empty until the user checks a model", () => {
  assert.deepEqual(selectModels(EMPTY_SELECTION, []), EMPTY_SELECTION);
  assert.deepEqual(selectModels(EMPTY_SELECTION, ["a", "b"]), {
    ids: ["a", "b"],
    defaultId: "a",
  });
});
test("unchecking the default picks a remaining model and clearing removes the default", () => {
  assert.deepEqual(selectModels({ ids: ["a", "b"], defaultId: "b" }, ["a"]), {
    ids: ["a"],
    defaultId: "a",
  });
  assert.deepEqual(
    selectModels({ ids: ["a"], defaultId: "a" }, []),
    EMPTY_SELECTION,
  );
});
test("search-result selection retains the user's default and deduplicates overlapping batches", () => {
  assert.deepEqual(
    selectModels({ ids: ["a", "b"], defaultId: "b" }, ["a", "b", "b", "c"]),
    { ids: ["a", "b", "c"], defaultId: "b" },
  );
});
test("bulk selection cannot exceed the native limit", () => {
  const selected = selectModels(
    EMPTY_SELECTION,
    Array.from({ length: 230 }, (_, i) => `model-${i}`),
  );
  assert.equal(selected.ids.length, 200);
  assert.equal(selected.defaultId, "model-0");
});
