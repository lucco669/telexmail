-- Public scores contain only the operator's chosen alias and derived run statistics.
-- The site's server verifies the run before inserting; browsers have no write access.
create table if not exists public.night_shift_scores (
  id uuid primary key,
  operator text not null check (char_length(operator) between 2 and 20),
  score integer not null check (score between 1 and 100000),
  delivered integer not null check (delivered between 1 and 100),
  clean integer not null check (clean between 0 and delivered),
  best_streak integer not null check (best_streak between 0 and 100),
  missed integer not null check (missed between 0 and 3),
  reaction_ms integer not null check (reaction_ms between 0 and 90000),
  writing_ms integer not null check (writing_ms between 1 and 90000),
  queue_ms integer not null check (queue_ms between 0 and 90000),
  wpm numeric(6,1) not null check (wpm > 0 and wpm <= 350),
  accuracy numeric(4,1) not null check (accuracy between 0 and 100),
  duration_seconds numeric(6,3) not null check (duration_seconds > 0 and duration_seconds <= 90),
  created_at timestamptz not null default now()
);
alter table public.night_shift_scores enable row level security;
revoke all on public.night_shift_scores from anon, authenticated, service_role;
grant select, insert on public.night_shift_scores to service_role;
create index if not exists night_shift_scores_score_idx on public.night_shift_scores (score desc, created_at);
create index if not exists night_shift_scores_created_idx on public.night_shift_scores (created_at desc);

create or replace function public.night_shift_leaderboard(p_sort text default 'score', p_period text default 'all')
returns setof public.night_shift_scores language sql stable security invoker set search_path = public as $$
  select winners.* from (
    select distinct on (lower(operator)) * from public.night_shift_scores
    where p_period = 'all'
      or (p_period = 'week' and created_at >= now() - interval '7 days')
      or (p_period = 'today' and created_at >= (date_trunc('day', now() at time zone 'UTC') at time zone 'UTC'))
    order by lower(operator),
      case when p_sort = 'speed' then wpm end desc nulls last,
      case when p_sort = 'reaction' then reaction_ms end asc nulls last,
      score desc, delivered desc, accuracy desc, created_at asc
  ) winners
  order by
    case when p_sort = 'speed' then wpm end desc nulls last,
    case when p_sort = 'reaction' then reaction_ms end asc nulls last,
    score desc, delivered desc, accuracy desc, created_at asc
  limit 20;
$$;
revoke all on function public.night_shift_leaderboard(text, text) from public, anon, authenticated;
grant execute on function public.night_shift_leaderboard(text, text) to service_role;

-- Rate limits per network: a keyed hash of the address (never the address) and an action, with
-- a count per fixed window. Rows older than two days are cleared as new ones come in.
create table if not exists public.night_shift_limits (
  bucket text not null check (char_length(bucket) <= 64),
  window_start timestamptz not null,
  hits integer not null default 0,
  primary key (bucket, window_start)
);
alter table public.night_shift_limits enable row level security;
revoke all on public.night_shift_limits from anon, authenticated, service_role;
grant select, insert, update, delete on public.night_shift_limits to service_role;
create index if not exists night_shift_limits_window_idx on public.night_shift_limits (window_start);

create or replace function public.night_shift_take(p_bucket text, p_limit integer, p_window_seconds integer)
returns boolean language plpgsql volatile security invoker set search_path = public as $$
declare
  v_window timestamptz := to_timestamp(floor(extract(epoch from now()) / greatest(p_window_seconds, 1)) * greatest(p_window_seconds, 1));
  v_hits integer;
begin
  insert into public.night_shift_limits as l (bucket, window_start, hits) values (p_bucket, v_window, 1)
  on conflict (bucket, window_start) do update set hits = l.hits + 1
  returning hits into v_hits;
  if random() < 0.02 then
    delete from public.night_shift_limits where window_start < now() - interval '2 days';
  end if;
  return v_hits <= p_limit;
end;
$$;
revoke all on function public.night_shift_take(text, integer, integer) from public, anon, authenticated;
grant execute on function public.night_shift_take(text, integer, integer) to service_role;
