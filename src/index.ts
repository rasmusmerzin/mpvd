import { createConnection } from "node:net";
import { MPVD_SOCK } from "./env.js";
import * as path from "node:path";

export interface PlaylistItem {
  filename: string;
  id: number;
  current?: boolean;
}

export function send(...command: (string | number | boolean)[]): Promise<any> {
  return new Promise((resolve, reject) => {
    const payload = JSON.stringify({ command }) + "\n";
    const socket = createConnection(MPVD_SOCK, () => {
      socket.write(payload);
      socket.end();
    });
    let buf = "";
    socket.on("data", (chunk) => (buf += chunk));
    socket.on("end", () => resolve(JSON.parse(buf.split("\n")[0])));
    socket.on("error", reject);
  });
}

export async function getPlaylist(): Promise<PlaylistItem[]> {
  const response = await send("get_property", "playlist");
  return response.data;
}

export async function getPause(): Promise<boolean> {
  const response = await send("get_property", "pause");
  return response.data;
}

export async function setPause(paused: boolean) {
  await send("set_property", "pause", paused);
}

export async function playAtIndex(index: number) {
  await send("playlist-play-index", index - 1);
}

export async function pushToPlaylist(file: string) {
  await send("loadfile", path.resolve(file), "append-play");
}

export async function goToNext() {
  await send("playlist-next");
}

export async function goToPrev() {
  await send("playlist-prev");
}

export async function moveInPlaylist(from: number, to: number) {
  console.log({ from, to });
  if (from === to) return;
  else if (from < to) await send("playlist-move", from - 1, to);
  else if (from > to) await send("playlist-move", from - 1, to - 1);
}
