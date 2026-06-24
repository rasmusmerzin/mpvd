import { createConnection } from "node:net";
import { resolveTilde } from "./find.js";
import { spawn } from "node:child_process";
import { readFile, rm, writeFile } from "node:fs/promises";
import { homedir } from "node:os";

export interface PlaylistItem {
  filename: string;
  id: number;
  current?: boolean;
}

export const MPVD_SOCK =
  process.env.MPVD_SOCK ||
  (process.env.XDG_RUNTIME_DIR || process.env.HOME) + "/mpvd.sock";

export const MPVD_PID = MPVD_SOCK.replace(/[^\/]+$/, "mpvd.pid");

export async function startDaemon(): Promise<boolean> {
  const pid = await getDaemonPID();
  if (pid !== null) return false;
  const child = spawn(
    "mpv",
    ["--idle", "--no-video", `--input-ipc-server=${MPVD_SOCK}`],
    {
      cwd: homedir(),
      detached: true,
      stdio: ["ignore", "ignore", "ignore"],
    },
  );
  child.unref();
  await writeFile(MPVD_PID, child.pid + "\n");
  return true;
}

export async function killDaemon(): Promise<boolean> {
  let success = true;
  const pid = await getDaemonPID();
  try {
    process.kill(pid!);
  } catch (e) {
    success = false;
  }
  try {
    await rm(MPVD_SOCK);
  } catch (e) {}
  try {
    await rm(MPVD_PID);
  } catch (e) {}
  return success;
}

export async function getDaemonPID(): Promise<number | null> {
  let pidText = "";
  try {
    pidText = await readFile(MPVD_PID, "utf-8");
  } catch (e) {
    return null;
  }
  const pid = parseInt(pidText.trim());
  return isNaN(pid) ? null : pid;
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

export async function getPosition(): Promise<number> {
  const response = await send("get_property", "playlist-pos");
  return response.data + 1;
}

export async function getDuration(): Promise<number> {
  const response = await send("get_property", "duration");
  return response.data;
}

export async function getTime(): Promise<number> {
  const response = await send("get_property", "time-pos");
  return response.data;
}

export async function getTimeString(): Promise<string> {
  const posSecs = (await getTime()) | 0;
  const durSecs = (await getDuration()) | 0;
  const mm = String((posSecs / 60) | 0).padStart(2, "0");
  const ss = String(posSecs % 60).padStart(2, "0");
  const MM = String((durSecs / 60) | 0).padStart(2, "0");
  const SS = String(durSecs % 60).padStart(2, "0");
  return `${mm}:${ss}/${MM}:${SS}`;
}

export async function getPause(): Promise<boolean> {
  const response = await send("get_property", "pause");
  return response.data;
}

export async function setPause(paused: boolean) {
  await send("set_property", "pause", paused);
}

export async function togglePause() {
  await setPause(!(await getPause()));
}

export async function playAtIndex(index: number) {
  await send("playlist-play-index", index - 1);
}

export async function pushToPlaylist(file: string) {
  await send("loadfile", resolveTilde(file), "append-play");
}

export async function goToNext() {
  await send("playlist-next");
}

export async function goToPrev() {
  await send("playlist-prev");
}

export async function moveInPlaylist(from: number, to: number) {
  if (from === to) return;
  else if (from < to) await send("playlist-move", from - 1, to);
  else if (from > to) await send("playlist-move", from - 1, to - 1);
}

export async function removeFromPlaylist(index: number) {
  await send("playlist-remove", index - 1);
}
