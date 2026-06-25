import { send } from "./index.js";
import { resolveTilde } from "./find.js";

export interface PlaylistItem {
  filename: string;
  id: number;
  current?: boolean;
}

export function getPlaylist(): Promise<PlaylistItem[]> {
  return send(["get_property", "playlist"]);
}

export async function getPosition(): Promise<number> {
  const position = await send(["get_property", "playlist-pos"]);
  return position + 1;
}

export function getDuration(): Promise<number> {
  return send(["get_property", "duration"]);
}

export function getTime(): Promise<number> {
  return send(["get_property", "time-pos"]);
}

export async function getTimeString(): Promise<string> {
  return formatTimeString(await getTime(), await getDuration());
}

export function formatTimeString(time: number, duration: number): string {
  const posSecs = time | 0;
  const durSecs = duration | 0;
  const mm = String((posSecs / 60) | 0).padStart(2, "0");
  const ss = String(posSecs % 60).padStart(2, "0");
  const MM = String((durSecs / 60) | 0).padStart(2, "0");
  const SS = String(durSecs % 60).padStart(2, "0");
  return `${mm}:${ss}/${MM}:${SS}`;
}

export function getPause(): Promise<boolean> {
  return send(["get_property", "pause"]);
}

export function setPause(paused: boolean) {
  return send(["set_property", "pause", paused]);
}

export async function togglePause() {
  return setPause(!(await getPause()));
}

export function playAtIndex(index: number) {
  return send(["playlist-play-index", index - 1]);
}

export function pushToPlaylist(file: string) {
  return send(["loadfile", resolveTilde(file), "append-play"]);
}

export function insertNext(file: string) {
  return send(["loadfile", resolveTilde(file), "insert-next"]);
}

export function goToNext() {
  return send(["playlist-next"]);
}

export function goToPrev() {
  return send(["playlist-prev"]);
}

export function moveInPlaylist(from: number, to: number) {
  if (from === to) return;
  else if (from < to) return send(["playlist-move", from - 1, to]);
  else if (from > to) return send(["playlist-move", from - 1, to - 1]);
}

export function removeFromPlaylist(index: number) {
  return send(["playlist-remove", index - 1]);
}

export function seek(seconds: number) {
  return send(["seek", seconds]);
}
