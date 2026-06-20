export function shuffle<T>(arr: T[]): T[] {
  const clone = [...arr];
  const shuffled = [];
  while (clone.length) {
    const index = Math.floor(Math.random() * clone.length);
    const [item] = clone.splice(index, 1);
    shuffled.push(item);
  }
  return shuffled;
}
