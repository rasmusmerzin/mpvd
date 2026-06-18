import { createConnection } from "node:net";

const socketPath =
  (process.env.XDG_RUNTIME_DIR || process.env.HOME) + "/mpvd.sock";

function send(...command: (string | number)[]) {
  return new Promise((resolve, reject) => {
    const payload = JSON.stringify({ command }) + "\n";
    const socket = createConnection(socketPath, () => {
      socket.write(payload);
      socket.end();
    });
    let buf = "";
    socket.on("data", (chunk) => (buf += chunk));
    socket.on("end", () => resolve(JSON.parse(buf)));
    socket.on("error", reject);
  });
}

console.log(await send(...process.argv.slice(2)));
