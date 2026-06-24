import { getPause, getPlaylist } from "./index.js";

export async function printPlaylist({
  plain = false,
  full = false,
}: { plain?: boolean; full?: boolean } = {}) {
  const pause = await getPause();
  const playlist = await getPlaylist();
  return playlist
    .map(({ filename, current }, i) => {
      let id = String(i + 1).padStart(4, " ");
      if (process.stdout.isTTY) id = `\x1b[2m${id}\x1b[m`;
      const cursor = current ? (pause ? "-" : "*") : " ";
      let name = full ? filename : /[^\/]+$/.exec(filename)![0];
      if (current && process.stdout.isTTY && !plain)
        name = `\x1b[32m${name}\x1b[m`;
      if (plain) return name;
      return `${id} ${cursor} ${name}`;
    })
    .join("\n");
}
