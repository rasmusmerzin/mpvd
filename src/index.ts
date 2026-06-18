import { createConnection } from "node:net";

export const SOCKET_PATH =
  (process.env.XDG_RUNTIME_DIR || process.env.HOME) + "/mpvd.sock";

export function send(...command: (string | number)[]) {
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

export async function list() {}
