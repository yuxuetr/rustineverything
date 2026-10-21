"""把 Docusaurus 博客转成本站文章目录（docs/BLOG_MIGRATION.md §5）。

用法：python3 scripts/migrate_docusaurus.py <docusaurus data/blog> <assets/posts>

每篇写到 `<assets/posts>/<slug>/index.mdx`，引用到的图片拷进同一目录。可重复运行：
只覆盖本脚本生成的文章目录。依赖 PyYAML。结束时打印每篇的警告。
"""
import hashlib
import os
import re
import shutil
import sys
import urllib.parse
import urllib.request
from typing import Iterator

import yaml

DATED_NAME = re.compile(r"^(\d{4})-(\d{2})-(\d{2})-(.+?)(\.mdx?)?$")
FENCE = re.compile(r"^(\s*)(`{3,}|~{3,})(.*)$")
IMAGE = re.compile(r"""!\[([^\]]*)\]\(([^)\s]+)((?:\s+(?:"[^"]*"|'[^']*'))?)\)""")
POST_LINK = re.compile(r"\]\((?:\.\.?/)*(\d{4}-\d{2}-\d{2}-[^/)#]+?)(?:/index)?\.mdx?(#[^)]*)?\)")
MDX_IMPORT = re.compile(r"""^\s*(import\s.*\bfrom\s+['"]|import\s+['"]|export\s+(const|let|function|default)\b)""")
MDX_COMMENT = re.compile(r"\{/\*.*?\*/\}")
# 旧站 ColorText 组件改用本站同类组件（属性同为 text）
COLOR = {"RedText": "Pink", "BlueText": "Blue", "GreenText": "Green", "YellowText": "Yellow", "PurpleText": "Purple"}
COLOR_TAG = re.compile(r"<(/?)(" + "|".join(COLOR) + r")\b")
# 独占一行的组件会被 CommonMark 当成 HTML 块，吞掉后面直到空行的内容；
# 包一层 `**`（颜色组件本就是粗体）让它留在段落里
LONE_COMPONENT = re.compile(r"^(\s*(?:[-*+]\s+|\d+\.\s+|>\s*)*)(<[A-Z][A-Za-z]* text=\"[^\"]*\"\s*/>)\s*$")
VIDEO_EXT = (".mp4", ".webm", ".mov")
# 个别文章的缩进 Docusaurus 能容忍、CommonMark 不行，逐篇修正（源名 → [(正则, 替换)]）
FIXES: dict[str, list[tuple[str, str]]] = {
  # 有序列表项缩进 3 格，子项只缩进 5 格（比内容列少 1 格），子项与代码块会并进上一行
  "2023-10-30-ts-decorator.mdx": [(r"^   (\d+\. )", r"  \1")],
}
KEPT_KEYS = {"title", "description", "tags", "keywords", "date", "publishedAt", "updatedAt", "image"}


def new_slug(old: str) -> str:
  return old.replace(".", "-")


def sources(root: str) -> Iterator[tuple[str, str]]:
  """(源名, index 文件路径)：单文件 `X.mdx` 与目录 `X/index.mdx` 两种写法。"""
  for name in sorted(os.listdir(root)):
    path = os.path.join(root, name)
    if os.path.isdir(path):
      for index in ("index.mdx", "index.md"):
        if os.path.isfile(os.path.join(path, index)):
          yield name, os.path.join(path, index)
          break
    elif name.endswith((".md", ".mdx")):
      yield name, path


def split_frontmatter(text: str) -> tuple[dict, str]:
  if text.startswith("---\n"):
    end = text.find("\n---", 4)
    if end > 0:
      return yaml.safe_load(text[4:end]) or {}, text[end + 4 :].lstrip("\n")
  return {}, text


def prose_segments(body: str) -> Iterator[tuple[bool, str]]:
  """按行切分正文：(是否在代码块外, 行)。代码块内原样保留。

  结束标记的缩进改成与开始标记一致：旧文有 Tab 缩进的结束标记，Docusaurus 认，
  CommonMark 不认（超过 3 列），后文会全部变成代码。
  """
  marker, indent = "", ""
  for line in body.split("\n"):
    m = FENCE.match(line)
    if not marker and m:
      indent, marker = m.group(1), m.group(2)
      yield False, line
    elif marker:
      if m and m.group(2)[0] == marker[0] and len(m.group(2)) >= len(marker) and not m.group(3).strip():
        line = indent + m.group(2)
        marker = ""
      yield False, line
    else:
      yield True, line


class Post:
  def __init__(self, name: str, src: str, out_root: str, slugs: dict[str, str]):
    m = DATED_NAME.match(name)
    if not m:
      raise ValueError("文件名不是 YYYY-MM-DD-slug")
    year, month, day, old = m.group(1), m.group(2), m.group(3), m.group(4)
    self.name = name
    self.src = src
    self.file_date = f"{year}-{month}-{day}"
    self.slug = new_slug(old)
    self.out_dir = os.path.join(out_root, self.slug)
    self.slugs = slugs
    self.copied: dict[str, str] = {}
    self.warnings: list[str] = []

  def convert(self) -> None:
    fm, body = split_frontmatter(open(self.src, encoding="utf-8").read())
    for pattern, repl in FIXES.get(self.name, []):
      body = re.sub(pattern, repl, body, flags=re.M)
    if os.path.isdir(self.out_dir):
      shutil.rmtree(self.out_dir)
    os.makedirs(self.out_dir)
    out_body = self.convert_body(body)
    out_fm = self.convert_frontmatter(fm)
    text = "---\n" + yaml.safe_dump(out_fm, allow_unicode=True, sort_keys=False, width=1000) + "---\n\n" + out_body
    with open(os.path.join(self.out_dir, "index.mdx"), "w", encoding="utf-8") as f:
      f.write(text.rstrip("\n") + "\n")

  def convert_frontmatter(self, fm: dict) -> dict:
    out: dict = {"title": str(fm.get("title", "")).strip()}
    if not out["title"]:
      self.warnings.append("缺 title")
    if fm.get("description"):
      out["description"] = str(fm["description"]).strip()
    out["date"] = str(fm.get("date") or fm.get("publishedAt") or self.file_date)[:10]
    if fm.get("updatedAt") and str(fm["updatedAt"])[:10] != out["date"]:
      out["updated"] = str(fm["updatedAt"])[:10]
    for key in ("tags", "keywords"):
      if fm.get(key):
        out[key] = fm[key]
    if fm.get("image"):
      local = self.copy_local(str(fm["image"]))
      if local:
        out["image"] = f"/posts/{self.slug}/{local}"
      else:
        self.warnings.append(f"封面图找不到，已去掉：{fm['image']}")
    dropped = sorted(set(fm) - KEPT_KEYS - {"authors", "hide_table_of_contents"})
    if dropped:
      self.warnings.append(f"去掉 frontmatter：{', '.join(dropped)}")
    return out

  def convert_body(self, body: str) -> str:
    out: list[str] = []
    skip_depth = 0
    for prose, line in prose_segments(body):
      if not prose:
        out.append(line)
        continue
      # import / export 行（含跨行的 `export const meta = { ... }`）
      if skip_depth > 0:
        skip_depth += line.count("{") - line.count("}")
        continue
      stripped = line.strip()
      if MDX_IMPORT.match(line):
        skip_depth = line.count("{") - line.count("}")
        continue
      # 行首 Tab 是作者的一级列表缩进；CommonMark 按 4 列算，嵌套列表会变成缩进代码块
      line = re.sub(r"^[ \t]+", lambda m: m.group(0).replace("\t", "  "), line)
      line = MDX_COMMENT.sub("", line)
      if not line.strip() and stripped:
        continue
      line = COLOR_TAG.sub(lambda m: f"<{m.group(1)}{COLOR[m.group(2)]}", line)
      line = LONE_COMPONENT.sub(r"\1**\2**", line)
      line = IMAGE.sub(self.rewrite_image, line)
      line = POST_LINK.sub(self.rewrite_post_link, line)
      out.append(line)
    return "\n".join(out)

  def rewrite_post_link(self, m: re.Match) -> str:
    target = m.group(1)
    slug = self.slugs.get(target)
    if slug is None:
      self.warnings.append(f"互链目标不存在：{target}")
      return m.group(0)
    return f"](/blog/{slug}{m.group(2) or ''})"

  def rewrite_image(self, m: re.Match) -> str:
    alt, url, title = m.group(1), m.group(2), m.group(3)
    if url.startswith(("http://", "https://")):
      if urllib.parse.urlparse(url).path.lower().endswith(VIDEO_EXT):
        return f"[{alt or '视频'}]({url})"
      local = self.download(url)
    else:
      local = self.copy_local(url)
    if not local:
      return m.group(0)
    return f"![{alt}]({local}{title})"

  def copy_local(self, url: str) -> str | None:
    """相对图片：先按文章所在目录解析，再试共享的 `images/`。返回新目录里的文件名。"""
    path = urllib.parse.unquote(url.split("#")[0].split("?")[0])
    base = os.path.dirname(self.src)
    candidates = [os.path.normpath(os.path.join(base, path))]
    blog_root = base if os.path.basename(self.src) not in ("index.mdx", "index.md") else os.path.dirname(base)
    candidates.append(os.path.normpath(os.path.join(blog_root, "images", os.path.basename(path))))
    found = next((c for c in candidates if os.path.isfile(c)), None)
    if found is None:
      self.warnings.append(f"图片找不到：{url}")
      return None
    # 同一张图可能在共享 images/ 与文章目录各有一份，按内容去重
    with open(found, "rb") as f:
      digest = hashlib.sha256(f.read()).hexdigest()
    return self.place(digest, os.path.basename(found), lambda dest: shutil.copyfile(found, dest))

  def download(self, url: str) -> str | None:
    name = os.path.basename(urllib.parse.urlparse(url).path) or "image"
    try:
      req = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0 (blog migration)"})
      with urllib.request.urlopen(req, timeout=30) as resp:
        ctype = resp.headers.get("Content-Type", "")
        data = resp.read()
    except Exception as e:  # 网络错误只影响这一张图，记下来继续
      self.warnings.append(f"外链图片下载失败，保留原地址：{url}（{e}）")
      return None
    if not ctype.startswith("image/"):
      self.warnings.append(f"外链不是图片（{ctype}），保留原地址：{url}")
      return None
    if "." not in name:
      name += "." + ctype.split("/")[1].split(";")[0].replace("jpeg", "jpg").replace("svg+xml", "svg")
    return self.place(url, name, lambda dest: open(dest, "wb").write(data))

  def place(self, key: str, name: str, write) -> str:
    """把文件写进文章目录；不同来源撞名时加序号。"""
    if key in self.copied:
      return self.copied[key]
    name = re.sub(r"[^A-Za-z0-9._-]", "-", name)
    stem, ext = os.path.splitext(name)
    n = 1
    while os.path.exists(os.path.join(self.out_dir, name)):
      n += 1
      name = f"{stem}-{n}{ext}"
    write(os.path.join(self.out_dir, name))
    self.copied[key] = name
    return name


def main() -> int:
  if len(sys.argv) != 3:
    print(__doc__)
    return 2
  src_root, out_root = sys.argv[1], sys.argv[2]
  items = []
  for name, path in sources(src_root):
    m = DATED_NAME.match(name)
    if not m:
      print(f"跳过（不是文章）：{name}")
      continue
    items.append((name, path, m.group(4)))
  slugs = {re.sub(r"\.mdx?$", "", name): new_slug(old) for name, _, old in items}
  if len(set(slugs.values())) != len(slugs):
    print("新 slug 有重名，停止")
    return 1
  warned = 0
  for name, path, _ in items:
    post = Post(name, path, out_root, slugs)
    post.convert()
    for w in post.warnings:
      print(f"{post.slug}: {w}")
    warned += bool(post.warnings)
  print(f"完成：{len(items)} 篇，{warned} 篇有警告")
  return 0


if __name__ == "__main__":
  sys.exit(main())
