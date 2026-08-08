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

export type Contribution = {
  source: { kind: "tag_affinity" | "pole_similarity" | "anilist_prior" | "personal_axis" | "penalty"; axis?: string };
  value: number;
  detail: string;
};

export type Recommendation = {
  work_id: number;
  title: string;
  score: { total: number; contributions: Contribution[] };
  explanation: { reasons: Contribution[]; risks: Contribution[] };
};

export type TasteProfile = {
  history_size: number;
  confidence: number;
  mode: "sparse_history" | "sparse_favorites" | "clustered";
  tag_affinities: { name: string; value: number; confidence: number; observed_works: number }[];
  poles: { ordinal: number; member_count: number; dominant_tags: Tag[]; representative_work_ids: number[] }[];
  axes: { source: "prior" | "learned"; observed_works: number; weights: { axis: string; weight: number }[] };
};

export type ProfileSnapshot = { version: number; created_at_unix: number; profile: TasteProfile };
export type EvaluationReport = {
  cases: number;
  configuration: { random_seed: number; relevant_rating_threshold: number };
  engine: EvaluationResult;
  baselines: { name: "random" | "anilist_global_score" | "tag_overlap"; metrics: { median_rank: number; recall_at_10: number; recall_at_20: number; mean_reciprocal_rank: number }; target_ranks: { work_id: number; rank: number }[] }[];
};

export type EvaluationResult = {
  name: "watch_mind" | "watchmind" | "random" | "anilist_global_score" | "tag_overlap";
  metrics: { median_rank: number; recall_at_10: number; recall_at_20: number; mean_reciprocal_rank: number };
  target_ranks: { work_id: number; rank: number }[];
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
  remove: (id: number) => request<{ profile_version: number }>(`/api/library/${id}`, { method: "DELETE" }),
  rate: (id: number, rating: number, aspects: string[]) =>
    request<{ profile_version: number }>(`/api/library/${id}/rating`, {
      method: "PUT",
      body: JSON.stringify({ rating, aspects: aspects.map((axis) => ({ axis, credit: 1 })) }),
    }),
  event: (id: number, event: WatchEvent) =>
    request<void>(`/api/library/${id}/events`, { method: "POST", body: JSON.stringify(event) }),
  recommendations: () => request<{ profile_version: number; recommendations: Recommendation[] }>("/api/recommendations"),
  historicalRecommendations: (version: number) => request<{ profile_version: number; recommendations: Recommendation[] }>(`/api/profile/${version}/recommendations`),
  feedback: (id: number, helpful: boolean) => request<void>(`/api/recommendations/${id}/feedback`, { method: "POST", body: JSON.stringify({ helpful }) }),
  profile: () => request<ProfileSnapshot>("/api/profile"),
  profiles: () => request<ProfileSnapshot[]>("/api/profiles"),
  evaluation: () => request<EvaluationReport>("/api/evaluation"),
};
