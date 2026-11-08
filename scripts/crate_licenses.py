# Writes licenses/RUST_CRATES.md. Usage:
#   cargo metadata --format-version 1 --filter-platform x86_64-pc-windows-msvc | python scripts/crate_licenses.py > licenses/RUST_CRATES.md
import json, sys
m = json.load(sys.stdin)
pk = {p['id']: p for p in m['packages']}
nodes = {n['id']: n for n in m['resolve']['nodes']}
seen, stack = set(), [m['resolve']['root']]
while stack:
    i = stack.pop()
    for d in nodes[i]['deps']:
        if any(k['kind'] is None for k in d['dep_kinds']) and d['pkg'] not in seen:
            seen.add(d['pkg']); stack.append(d['pkg'])
rows = sorted((pk[i]['name'], pk[i]['version'], pk[i].get('license') or '', pk[i].get('repository') or '') for i in seen)
print("""# Rust libraries in ProxyCraft.exe

ProxyCraft.exe is statically linked with the open-source Rust libraries (crates) below, built for
64-bit Windows. Each is used unmodified, from <https://crates.io>, where its source code is published.
Where a crate offers a choice of licences, ProxyCraft uses it under the MIT or Apache-2.0 option.

Licence texts: `Apache-2.0.txt`, `BSL-1.0.txt`, `MPL-2.0.txt`, `Unicode-3.0.txt`, `OFL-1.1.txt` and
`Ubuntu-Font-Licence-1.0.txt` in this folder; the MIT, ISC and Zlib texts are at the end of this file.
The copyright holders of each crate are named in its source.

Regenerate this list after changing dependencies (see PROJECT_NOTES.txt).

| Crate | Version | Licence | Source |
|---|---|---|---|""")
for n, v, l, r in rows:
    print(f"| {n} | {v} | {l} | {r} |")
print(f"""
{len(rows)} crates.

## MIT License

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
associated documentation files (the "Software"), to deal in the Software without restriction,
including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense,
and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so,
subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial
portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT
LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN
NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY,
WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE
SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

## ISC License

Permission to use, copy, modify, and/or distribute this software for any purpose with or without fee
is hereby granted, provided that the above copyright notice and this permission notice appear in all
copies.

THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES WITH REGARD TO THIS SOFTWARE
INCLUDING ALL IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE
FOR ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES WHATSOEVER RESULTING FROM LOSS
OF USE, DATA OR PROFITS, WHETHER IN AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING
OUT OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

## Zlib License

This software is provided 'as-is', without any express or implied warranty. In no event will the
authors be held liable for any damages arising from the use of this software.

Permission is granted to anyone to use this software for any purpose, including commercial
applications, and to alter it and redistribute it freely, subject to the following restrictions:

1. The origin of this software must not be misrepresented; you must not claim that you wrote the
   original software. If you use this software in a product, an acknowledgment in the product
   documentation would be appreciated but is not required.
2. Altered source versions must be plainly marked as such, and must not be misrepresented as being
   the original software.
3. This notice may not be removed or altered from any source distribution.""")
