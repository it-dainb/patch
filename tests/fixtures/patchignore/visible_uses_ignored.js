import { ignoredApiJs } from "./ignored-dir/ignored_api";

export function visibleUsesIgnoredJs() {
  return ignoredApiJs();
}
