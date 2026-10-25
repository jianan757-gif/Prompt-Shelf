from pathlib import Path
import hashlib, json, sys, argparse
sys.stdout.reconfigure(encoding="utf-8")
root=Path(__file__).resolve().parents[1]
parser=argparse.ArgumentParser();parser.add_argument('--fixture',default='100x500',choices=['100x500','100x500-mixed']);args=parser.parse_args()
base=root/"fixtures"/args.fixture
m=json.loads((base/"manifest.json").read_text(encoding="utf-8"))
failures=[]
ids={a["id"] for a in m["assets"]}
for a in m["assets"]:
    p=base/a["original"]
    if not p.is_file() or hashlib.sha256(p.read_bytes()).hexdigest()!=a["sha256"]: failures.append(a["id"])
    thumb=base/a["thumb"]
    if not thumb.is_file(): failures.append(a["id"]+":thumb")
    elif hashlib.sha256(thumb.read_bytes()).hexdigest()!=a.get("thumb_sha256"): failures.append(a["id"]+":thumb_hash")
    if not thumb.resolve().is_relative_to((base/'cache'/'thumbs').resolve()) or thumb.resolve()==p.resolve(): failures.append(a['id']+':thumb_path')
refs=[a for c in m["cases"] for a in ([c["result"]] if c["result"] else [])+c["refs"]]
failures += [r for r in refs if r not in ids]
if len(set(a['sha256'] for a in m['assets']))!=500: failures.append('unique_original_hashes')
if len(ids)!=500 or len(set(c['id'] for c in m['cases']))!=100: failures.append('unique_ids')
if sum(not c['result'] and not c['refs'] for c in m['cases'])!=1: failures.append('one_empty_case')
if len(refs)!=501 or len(set(refs))!=500: failures.append('duplicate_reference_contract')
report=dict(kind="AUTO-FILE-INTEGRITY",cases=len(m["cases"]),physical_originals=len(m["assets"]),references=len(refs),unique_referenced=len(set(refs)),failures=failures,fixture_kind=m["fixture_kind"],real_photo_acceptance="pending")
(root/"evidence"/(args.fixture+"-integrity.json")).write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding="utf-8")
print(json.dumps(report,ensure_ascii=False))
raise SystemExit(bool(failures) or len(m["cases"])!=100 or len(m["assets"])!=500)
