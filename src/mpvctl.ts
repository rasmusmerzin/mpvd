import { program } from "commander";
import { createConnection } from "node:net";
import pkg from "../package.json" with { type: "json" };

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

program.name("mpvctl").description("MPV daemon control").version(pkg.version);

program
  .command("send")
  .description("Send IPC command")
  .arguments("<cmd...>")
  .action(async function (args: string[]) {
    console.log(await send(...args));
  });

program.parse();
