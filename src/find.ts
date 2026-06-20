import * as fs from "node:fs/promises";
import { homedir } from "node:os";
import * as path from "node:path";

/** Find matching files recursively */
export async function find(
  target: string,
  filter: (realpath: string) => boolean = () => true,
): Promise<string[]> {
  target = resolveTilde(target);
  const results: string[] = [];
  const entries = await fs.readdir(target, { withFileTypes: true });
  for (const entry of entries) {
    const fullPath = path.join(target, entry.name);
    if (entry.isDirectory()) results.push(...(await find(fullPath, filter)));
    else if (entry.isFile() && filter(fullPath)) results.push(fullPath);
  }
  return results;
}

export function resolveTilde(filePath: string) {
  if (filePath.startsWith("~")) return path.join(homedir(), filePath.slice(1));
  else return path.resolve(filePath);
}

const MPV_AUDIO_EXTS = new Set(
  "aac,ac3,aiff,ape,au,dts,eac3,flac,m4a,mka,mp3,oga,ogg,ogm,opus,thd,wav,wma,wv,tta".split(
    ",",
  ),
);

/** Checks if the given path is an audio file playable by mpv based on extension. */
export function isMpvPlayableAudio(filePath: string): boolean {
  const ext = path.extname(filePath).slice(1).toLowerCase();
  return MPV_AUDIO_EXTS.has(ext);
}
