import { resolveTilde } from "./find.js";

export const MPVD_SOCK = resolveTilde(
  process.env.MPVD_SOCK ||
    (process.env.XDG_RUNTIME_DIR || process.env.HOME) + "/mpvd.sock",
);

export const MPVD_PID = resolveTilde(
  process.env.MPVD_PID || MPVD_SOCK.replace(/[^\/]+$/, "mpvd.pid"),
);
