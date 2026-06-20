import React, { useEffect, useState } from "react";
import { Text, useInput } from "ink";
import * as path from "node:path";
import { useTerminalSize } from "./useTerminalSize.js";
import { find, isMpvPlayableAudio } from "./find.js";
import { shuffle } from "./shuffle.js";
import { unmount } from "./mpvd.js";
import { pushToPlaylist } from "./index.js";

export function PickerLine({
  selected,
  hover,
  columns,
  file,
}: {
  selected: boolean;
  hover: boolean;
  columns: number;
  file: string;
}) {
  const prefix = selected ? "* " : "  ";
  const full = prefix + path.basename(file).slice(0, columns - 2);
  return <Text inverse={hover}>{full.padEnd(columns, " ")}</Text>;
}

export function Picker() {
  const { columns, rows } = useTerminalSize();
  const [files, setFiles] = useState<string[]>([]);
  let [offset, setOffset] = useState(0);
  let [cursor, setCursor] = useState(0);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const listHeight = rows - 1;
  const maxOffset = files.length - listHeight;
  function changeOffset(update: number) {
    setOffset((offset = Math.max(0, Math.min(maxOffset, update))));
    changeCursor(cursor);
  }
  function changeCursor(pos: number) {
    const minCursor = offset;
    const maxCursor = offset + listHeight - 1;
    setCursor((cursor = Math.max(minCursor, Math.min(maxCursor, pos))));
  }
  useEffect(() => {
    find("~/Music", isMpvPlayableAudio).then(setFiles);
  }, []);
  useInput((input, key) => {
    if (key.downArrow || (input === "e" && key.ctrl)) changeOffset(offset + 1);
    else if (key.upArrow || (input === "y" && key.ctrl))
      changeOffset(offset - 1);
    else if (input === "d" && key.ctrl) changeOffset((offset + rows / 2) | 0);
    else if (input === "u" && key.ctrl) changeOffset(offset - ((rows / 2) | 0));
    else if (input === "r") setFiles(shuffle(files));
    else if (input === "q") process.exit();
    else if (input === "j") changeCursor(cursor + 1);
    else if (input === "k") changeCursor(cursor - 1);
    else if (input === "g") {
      changeOffset(0);
      changeCursor(0);
    } else if (input === "G") {
      changeOffset(maxOffset);
      changeCursor(files.length - 1);
    } else if (input === " ") {
      if (selected.has(files[cursor])) selected.delete(files[cursor]);
      else selected.add(files[cursor]);
      setSelected(new Set(selected));
    } else if (key.return) {
      unmount();
      Promise.all(
        Array.from(selected, async (file) => {
          await pushToPlaylist(file);
          console.log(file);
        }),
      );
    }
  });
  return (
    <>
      {files.slice(offset, offset + listHeight).map((file, i) => (
        <PickerLine
          key={file}
          file={file}
          selected={selected.has(file)}
          hover={cursor === i + offset}
          columns={columns}
        />
      ))}
    </>
  );
}
