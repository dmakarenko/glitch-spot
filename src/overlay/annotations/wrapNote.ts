// ABOUTME: Pure greedy word-wrap for caption text. Explicit newlines split lines;
// ABOUTME: a single word wider than maxWidth is hard-broken so no line overflows.

export function wrapNote(
  text: string,
  maxWidth: number,
  measure: (s: string) => number,
): string[] {
  const lines: string[] = [];
  for (const paragraph of text.split('\n')) {
    let line = '';
    for (const word of paragraph.split(' ').filter((w) => w !== '')) {
      if (measure(word) > maxWidth) {
        if (line) {
          lines.push(line);
          line = '';
        }
        let chunk = '';
        for (const ch of word) {
          if (chunk && measure(chunk + ch) > maxWidth) {
            lines.push(chunk);
            chunk = '';
          }
          chunk += ch;
        }
        line = chunk;
        continue;
      }
      const candidate = line ? `${line} ${word}` : word;
      if (measure(candidate) > maxWidth && line) {
        lines.push(line);
        line = word;
      } else {
        line = candidate;
      }
    }
    lines.push(line);
  }
  return lines;
}
