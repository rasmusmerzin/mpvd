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
  private socket!: Socket;
  private buffer = "";
  private listeners = new Set<AbortController>();
  private idSeq = 1;

  static connect(): Promise<MpvObserver> {
    return new Promise((resolve, reject) => {
      const observer = new MpvObserver();
      observer.socket = createConnection(MPVD_SOCK, () => resolve(observer));
      observer.socket.on("error", reject);
      observer.socket.on("end", () => reject(new Error("connection closed")));
      observer.socket.on("data", (chunk) => {
        observer.buffer += chunk;
        if (observer.buffer.includes("\n")) {
          const parts = observer.buffer.split("\n");
          observer.buffer = parts.pop() || "";
          const messages = parts.map(safeParse).filter((a) => a);
          for (const message of messages) {
            observer.dispatchEvent(
              message.event
                ? new CustomEvent("event", { detail: message })
                : new CustomEvent("data", { detail: message }),
            );
          }
        }
      });
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
