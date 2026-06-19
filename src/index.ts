import { spawn } from "node:child_process";
import { readFile, rm, stat, writeFile } from "node:fs/promises";
import { createConnection } from "node:net";

export interface PlaylistItem {
  filename: string;
  id: number;
  current?: boolean;
}

export const MPVD_SOCK =
  process.env.MPVD_SOCK ||
  (process.env.XDG_RUNTIME_DIR || process.env.HOME) + "/mpvd.sock";

export const MPVD_PID = MPVD_SOCK.replace(/[^\/]+$/, "mpvd.pid");

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

export async function spawnDaemon(): Promise<boolean> {
  const pidStat = await stat(MPVD_PID).catch(() => {});
  if (pidStat && pidStat.isFile()) return false;
  const child = spawn(
    "mpv",
    ["--idle", "--no-video", `--input-ipc-server=${MPVD_SOCK}`],
    {
      cwd: process.env.HOME,
      detached: true,
      stdio: ["ignore", "ignore", "ignore"],
    },
  );
  child.unref();
  await writeFile(MPVD_PID, child.pid + "\n");
  return true;
}

export async function killDaemon(): Promise<boolean> {
  let pidText = "";
  try {
    pidText = await readFile(MPVD_PID, "utf-8");
  } catch (e) {
    return false;
  }
  const pid = parseInt(pidText.trim());
  let success = true;
  try {
    process.kill(pid);
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
