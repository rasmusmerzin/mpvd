import { createConnection } from "node:net";

export interface PlaylistItem {
  filename: string;
  id: number;
  current?: boolean;
}

export const SOCKET_PATH =
  process.env.MPVD_SOCK ||
  (process.env.XDG_RUNTIME_DIR || process.env.HOME) + "/mpvd.sock";

export function send(...command: (string | number)[]): Promise<any> {
  return new Promise((resolve, reject) => {
    const payload = JSON.stringify({ command }) + "\n";
    const socket = createConnection(SOCKET_PATH, () => {
      socket.write(payload);
      socket.end();
    });
    let buf = "";
    socket.on("data", (chunk) => (buf += chunk));
    socket.on("end", () => resolve(JSON.parse(buf)));
    socket.on("error", reject);
  });
}

export async function list({ plain = false }: { plain?: boolean } = {}) {
  const playlist = await getPlaylist();
  return playlist
    .map(({ filename, current }, i) => {
      let id = String(i + 1).padStart(4, " ");
      if (!plain) id = `\x1b[2m${id}\x1b[m`;
      const cursor = current ? "*" : " ";
      let name = /[^\/]+$/.exec(filename)![0];
      if (current && !plain) name = `\x1b[32m${name}\x1b[m`;
      return `${id} ${cursor} ${name}`;
    })
    .join("\n");
}

export async function getPlaylist(): Promise<PlaylistItem[]> {
  const response = await send("get_property", "playlist");
  return response.data;
}
