// Plain-language explanations of updater failures. Every failure before installation leaves the
// installed version untouched: the download is held in memory and its signature is verified
// before anything is written.

export type UpdatePhase = "check" | "download" | "install";

export function explainUpdateError(message: string, phase: UpdatePhase): string {
  const m = message.toLowerCase();
  if (phase === "check") {
    if (m.includes("valid release json") || m.includes("404")) return "No update information is published yet, or GitHub couldn't be reached. Try again later.";
    if (m.includes("platform") && m.includes("not found")) return "The newest release has no package for this computer yet. Try again later.";
    return `Couldn't check for updates: ${message}`;
  }
  if (m.includes("signature") || m.includes("minisign") || m.includes("base64")) return "The download couldn't be verified as an official Salon Resource Manager release, so it was discarded. Nothing was changed.";
  if (m.includes("authentication failed") || m.includes("cancelled") || m.includes("canceled")) return "Installation needs your permission and was cancelled. Your current version is unchanged.";
  if (phase === "download" || m.includes("request") || m.includes("timed out") || m.includes("connection") || m.includes("decoding response body") || m.includes("status"))
    return "The download was interrupted. Nothing was changed. Check your connection and try again.";
  if (m.includes("mount point") || m.includes("permission denied") || m.includes("read-only")) return "The app couldn't replace its own file (it may be in a protected or read-only folder). Your current version is unchanged; download the update from GitHub instead.";
  return `The update couldn't be installed: ${message}. Your current version is unchanged.`;
}

/** "3.4 MB" */
export function fmtBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(0)} KB`;
  return `${(n / 1024 / 1024).toFixed(1)} MB`;
}
