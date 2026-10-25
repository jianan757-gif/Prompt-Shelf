"""Copy user-authorized files without modifying them; keep exact 100/500 workload."""
from pathlib import Path
from PIL import Image
import json,hashlib,shutil,argparse,sys
sys.stdout.reconfigure(encoding="utf-8")
parser=argparse.ArgumentParser();parser.add_argument("source",type=Path);args=parser.parse_args()
root=Path(__file__).resolve().parents[1]
base=root/"fixtures"/"100x500"
dest=root/"fixtures"/"100x500-mixed"
if dest.exists():raise SystemExit("Mixed directory exists; preserving it.")
source=args.source.resolve(strict=True)
files=sorted(p for p in source.iterdir() if p.is_file() and p.suffix.lower() in {".png",".jpg",".jpeg",".webp"})[:500]
m=json.loads((base/"manifest.json").read_text(encoding="utf-8"))
evidence=[]
for index,asset in enumerate(m["assets"]):
    if index<len(files):
        path=files[index];data=path.read_bytes();digest=hashlib.sha256(data).hexdigest()
        rel=Path("assets")/"用户样本"/(f"{index+1:03}-"+path.name)
        target=dest/rel;target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(data)
        with Image.open(path) as im:
            width,height=im.size
            thumb=im.copy();thumb.thumbnail((540,180),Image.Resampling.LANCZOS)
            thumb_rel=Path("cache")/"thumbs"/(digest+"-180-v1.png")
            (dest/thumb_rel).parent.mkdir(parents=True,exist_ok=True);thumb.save(dest/thumb_rel)
            asset.update(original=rel.as_posix(),thumb=thumb_rel.as_posix(),sha256=digest,thumb_sha256=hashlib.sha256((dest/thumb_rel).read_bytes()).hexdigest(),width=width,height=height,format=im.format.lower(),bytes=len(data),synthetic=False,has_icc=bool(im.info.get("icc_profile")),alpha="A" in im.getbands())
        evidence.append(dict(source=str(path),source_sha256=digest,copy_sha256=hashlib.sha256(target.read_bytes()).hexdigest(),source_unchanged=hashlib.sha256(path.read_bytes()).hexdigest()==digest))
    else:
        for key in ("original","thumb"):
            target=dest/asset[key];target.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(base/asset[key],target)
m["fixture_kind"]="mixed-user-screenshots-and-synthetic"
m["notes"]=[f"{len(files)} user-provided screenshots + {500-len(files)} synthetic images; original source files unmodified", "Real high-resolution photos and calibrated ICC/color acceptance remain pending"]
(dest/"manifest.json").write_text(json.dumps(m,ensure_ascii=False,indent=2),encoding="utf-8")
(root/"evidence"/"user-sample-copy-integrity.json").write_text(json.dumps(evidence,ensure_ascii=False,indent=2),encoding="utf-8")
print(json.dumps(dict(cases=len(m["cases"]),assets=len(m["assets"]),user_files=len(files),source_unchanged=all(x["source_unchanged"] for x in evidence),path=str(dest)),ensure_ascii=False))
