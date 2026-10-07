export interface ModelSelection {
  ids: string[];
  defaultId: string;
}
export const EMPTY_SELECTION: ModelSelection = { ids: [], defaultId: "" };
export const MAX_MODELS = 200;

// Removing the default must never leave an unselected model as the default.
export function selectModels(
  current: ModelSelection,
  ids: string[],
): ModelSelection {
  const unique = [...new Set(ids)].slice(0, MAX_MODELS);
  return {
    ids: unique,
    defaultId: unique.includes(current.defaultId)
      ? current.defaultId
      : (unique[0] ?? ""),
  };
}
