import { createConnection } from "node:net";
import { MPVD_SOCK } from "./index.js";

export class MpvError extends Error {}

export function send(
  command: (string | number | boolean)[],
  { raw = false }: { raw?: boolean } = {},
): Promise<any> {
  const request_id = nextRequestId();
  return new Promise((resolve, reject) => {
    const payload = JSON.stringify({ command, request_id }) + "\n";
    const socket = createConnection(MPVD_SOCK, () => {
      socket.write(payload);
      socket.end();
    });
    let buf = "";
    socket.on("data", (chunk) => (buf += chunk));
    socket.on("end", () => {
      const messages = buf.split("\n").map(safeParse);
      const response = messages.find((m) => m?.request_id === request_id);
      if (raw) resolve(response);
      else if (response.error && response.error !== "success") {
        reject(new MpvError(response.error));
      } else resolve(response.data);
    });
    socket.on("error", reject);
  });
}

function nextRequestId(): string {
  return `${Date.now()}:${performance.now()}`;
}

function safeParse(text: string) {
  try {
    return JSON.parse(text);
  } catch (e) {}
}
