import { createConnection, Socket } from "node:net";
import { MPVD_SOCK } from "./index.js";

export class MpvError extends Error {}

export function send(
  command: (string | number | boolean)[],
  { raw = false }: { raw?: boolean } = {},
): Promise<any> {
  return new Promise((resolve, reject) => {
    const request_id = nextRequestId();
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

export class MpvObserver extends EventTarget {
  private socket: Socket;
  private buffer = "";
  private listeners = new Set<AbortController>();
  private idSeq = 1;

  constructor() {
    super();
    this.socket = createConnection(MPVD_SOCK, () =>
      this.dispatchEvent(new Event("connect")),
    );
    this.socket.on("error", (error) =>
      this.dispatchEvent(new CustomEvent("error", { detail: error })),
    );
    this.socket.on("end", () => this.dispatchEvent(new Event("close")));
    this.socket.on("data", (chunk) => {
      this.buffer += chunk;
      if (this.buffer.includes("\n")) {
        const parts = this.buffer.split("\n");
        this.buffer = parts.pop() || "";
        const messages = parts.map(safeParse).filter((a) => a);
        for (const message of messages) {
          this.dispatchEvent(
            message.event
              ? new CustomEvent("event", { detail: message })
              : new CustomEvent("data", { detail: message }),
          );
        }
      }
    });
  }

  static connect(): Promise<MpvObserver> {
    return new Promise((resolve, reject) => {
      const observer = new MpvObserver();
      const control = new AbortController();
      const { signal } = control;
      function onConnect() {
        resolve(observer);
        control.abort();
      }
      function onClose() {
        reject(new Error("connection closed"));
        control.abort();
      }
      function onError(event: Event) {
        const { detail } = event as CustomEvent<Error>;
        reject(detail);
        control.abort();
      }
      observer.addEventListener("connect", onConnect, { signal });
      observer.addEventListener("close", onClose, { signal });
      observer.addEventListener("error", onError, { signal });
    });
  }

  observe<T = any>(property: string, listener: (msg: T) => any): () => void {
    const control = new AbortController();
    this.listeners.add(control);
    const id = this.idSeq++;
    const command = ["observe_property", id, property];
    const payload = JSON.stringify({ command }) + "\n";
    this.socket.write(payload);
    this.addEventListener("event", (event) => {
      const { detail } = event as CustomEvent<{
        event: string;
        id: number;
        data: T;
      }>;
      if (detail.event === "property-change" && detail.id === id)
        listener(detail.data);
    });
    return () => {
      const command = ["unobserve_property", id];
      const payload = JSON.stringify({ command }) + "\n";
      this.socket.write(payload);
      this.listeners.delete(control);
      control.abort();
    };
  }

  close() {
    this.socket.end();
    this.listeners.forEach((control) => control.abort());
    this.listeners.clear();
  }
}

function nextRequestId(): string {
  return `${Date.now()}:${performance.now()}`;
}

function safeParse(text: string) {
  try {
    return JSON.parse(text);
  } catch (e) {}
}
