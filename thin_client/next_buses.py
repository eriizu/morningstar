import json
import sys
from datetime import datetime, timezone
from typing import Any, Dict, List, Optional, Tuple
from urllib.request import urlopen, Request
from urllib.error import URLError, HTTPError

DEFAULT_URL = "http://localhost:3000/stop/Parc%20du%20Bel-Air"
MAX_DEST_LEN = 28  # truncate destination names to keep table tidy


def parse_iso(dt_str: Optional[str]) -> Optional[datetime]:
    if not dt_str:
        return None
    if dt_str.endswith("Z"):
        dt_str = dt_str[:-1] + "+00:00"
    try:
        return datetime.fromisoformat(dt_str)
    except ValueError:
        return None


def arrival_time(item: Dict[str, Any]) -> Optional[datetime]:
    return parse_iso(item.get("expected_arrival")) or parse_iso(
        item.get("aimed_arrival")
    )


def fetch_json(url: str) -> List[Dict[str, Any]]:
    req = Request(url, headers={"Accept": "application/json"})
    with urlopen(req, timeout=10) as resp:
        charset = resp.headers.get_content_charset() or "utf-8"
        body = resp.read().decode(charset)
        return json.loads(body)


def human_time(dt: datetime) -> str:
    return dt.astimezone().strftime("%H:%M")


def minutes_from_now(target: datetime, now: datetime) -> str:
    # Round down negative minutes to 0 for "due"
    delta = int((target - now).total_seconds() // 60)
    if delta <= 0:
        return "due"
    if delta == 1:
        return "1 min"
    return f"{delta} mins"


def truncate(text: str, length: int) -> str:
    if len(text) <= length:
        return text
    if length <= 1:
        return text[:length]
    return text[: length - 1] + "…"


def build_table(rows: List[List[str]], headers: List[str]) -> str:
    # Compute column widths
    cols = len(headers)
    widths = [len(h) for h in headers]
    for r in rows:
        for i in range(cols):
            widths[i] = max(widths[i], len(r[i]))

    # Cap a bit to avoid overly wide columns
    widths = [min(w, 40) for w in widths]

    def fmt_row(cells: List[str]) -> str:
        parts = []
        for i, cell in enumerate(cells):
            parts.append(cell.ljust(widths[i]))
        return "  ".join(parts)

    sep = "  ".join("-" * w for w in widths)
    out = []
    out.append(fmt_row(headers))
    out.append(sep)
    for r in rows:
        out.append(fmt_row(r))
    return "\n".join(out)


def main() -> None:
    url = sys.argv[1] if len(sys.argv) > 1 else DEFAULT_URL

    try:
        data = fetch_json(url)
    except HTTPError as e:
        print(f"HTTP error {e.code} while fetching {url}: {e.reason}")
        sys.exit(1)
    except URLError as e:
        print(f"Network error while fetching {url}: {e.reason}")
        sys.exit(1)
    except json.JSONDecodeError as e:
        print(f"Failed to parse JSON from {url}: {e}")
        sys.exit(1)
    except Exception as e:
        print(f"Unexpected error: {e}")
        sys.exit(1)

    now = datetime.now(timezone.utc).astimezone()

    enriched: List[Tuple[datetime, Dict[str, Any]]] = []
    for it in data:
        arr = arrival_time(it)
        if arr and arr >= now:
            enriched.append((arr, it))

    enriched.sort(key=lambda x: x[0])
    next_five = enriched[:5]

    if not next_five:
        print("No upcoming buses found.")
        return

    rows: List[List[str]] = []
    headers = ["Expected at", "In", "Destination", "Status", "Stops to dest.", "Theorical"]

    for arr, it in next_five:
        dest = truncate(it.get("destination") or "Unknown", MAX_DEST_LEN)
        status = it.get("status") or "—"
        stops = it.get("stops_to_destination")
        aimed = parse_iso(it.get("aimed_arrival"))
        rows.append(
            [
                human_time(arr),
                minutes_from_now(arr, now),
                dest,
                status,
                str(stops) if stops is not None else "—",
                human_time(aimed) if aimed else "—",
            ]
        )

    print("Next 5 buses")
    print(build_table(rows, headers))


if __name__ == "__main__":
    main()
