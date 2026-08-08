export type Tag = { name: string; weight: number };

export type Work = {
  id: number;
  title: string;
  global_score: number | null;
  runtime_minutes?: number;
  format?: string;
  release_year?: number;
  studios?: string[];
  tags: Tag[];
};

export type WatchEvent = {
  kind: "completed" | "dropped" | "rewatched";
  work_id: number;
  progress?: { position: number; total: number };
};

export type CompleteWork = {
  work: Work;
  library: { work_id: number; comment: string | null } | null;
  rating: { work_id: number; rating: number; aspects: { axis: string; credit: number }[] } | null;
  events: WatchEvent[];
};

type CatalogResponse = { works: Work[]; from_cache: boolean };

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(path, {
    ...init,
    headers: { "Content-Type": "application/json", ...init?.headers },
  });
  if (!response.ok) {
    const payload = (await response.json().catch(() => null)) as { error?: string } | null;
    throw new Error(payload?.error ?? `La requête a échoué (${response.status}).`);
  }
  return response.status === 204 ? (undefined as T) : (response.json() as Promise<T>);
}

export const api = {
  library: () => request<CompleteWork[]>("/api/library"),
  search: (query: string) => request<CatalogResponse>(`/api/anime/search?q=${encodeURIComponent(query)}`),
  add: (work: Work, comment: string | null = null) =>
    request<void>(`/api/library/${work.id}`, { method: "PUT", body: JSON.stringify({ work, comment }) }),
  updateComment: (work: Work, comment: string | null) =>
    request<void>(`/api/library/${work.id}`, { method: "PUT", body: JSON.stringify({ work, comment }) }),
  rate: (id: number, rating: number, aspects: string[]) =>
    request<{ profile_version: number }>(`/api/library/${id}/rating`, {
      method: "PUT",
      body: JSON.stringify({ rating, aspects: aspects.map((axis) => ({ axis, credit: 1 })) }),
    }),
  event: (id: number, event: WatchEvent) =>
    request<void>(`/api/library/${id}/events`, { method: "POST", body: JSON.stringify(event) }),
};
