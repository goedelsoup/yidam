import { tableFromIPC, Table, Vector } from 'apache-arrow';

export interface SearchResult {
  score: number;
  path: string;
  class: string;
  label: string;
  text: string;
}

function cosineSimilarity(a: Float32Array, b: Float32Array): number {
  let dot = 0, na = 0, nb = 0;
  for (let i = 0; i < a.length; i++) {
    dot += a[i] * b[i];
    na += a[i] * a[i];
    nb += b[i] * b[i];
  }
  const denom = Math.sqrt(na) * Math.sqrt(nb);
  return denom === 0 ? 0 : dot / denom;
}

export class CorpusIndex {
  private table: Table;

  private constructor(table: Table) {
    this.table = table;
  }

  static fromArrow(buffer: Uint8Array): CorpusIndex {
    return new CorpusIndex(tableFromIPC(buffer));
  }

  get nodeCount(): number {
    return this.table.numRows;
  }

  search(queryVec: Float32Array, k = 5): SearchResult[] {
    const vectorCol = this.table.getChild('vector');
    const pathCol = this.table.getChild('path');
    const classCol = this.table.getChild('class');
    const labelCol = this.table.getChild('label');
    const textCol = this.table.getChild('text');

    if (!vectorCol || this.table.numRows === 0) return [];

    const scores: { score: number; idx: number }[] = [];
    for (let i = 0; i < this.table.numRows; i++) {
      const vec = (vectorCol.get(i) as Vector).toArray() as Float32Array;
      scores.push({ score: cosineSimilarity(queryVec, vec), idx: i });
    }

    scores.sort((a, b) => b.score - a.score);

    return scores.slice(0, k).map(({ score, idx }) => ({
      score,
      path: String(pathCol?.get(idx) ?? ''),
      class: String(classCol?.get(idx) ?? ''),
      label: String(labelCol?.get(idx) ?? ''),
      text: String(textCol?.get(idx) ?? ''),
    }));
  }
}
