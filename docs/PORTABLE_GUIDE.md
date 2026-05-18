# HWP CRUD via rhwp — Portable Guide

다른 머신·다른 프로젝트·다른 Claude 세션에서 HWP/HWPX 문서를 프로그래밍적으로 다룰 때 컨텍스트로 쓰는 자체완결 가이드.

한컴독스 호환까지 검증된 워크어라운드 8개와 즉시 쓸 수 있는 hwp-maker 의 GitHub 위치를 담는다. **이 문서를 새 머신/세션의 컨텍스트로 주면 Claude 가 cold start 로도 같은 결과를 재현할 수 있다.**

## 0. 위치

```
https://github.com/0xMegg/hwp-maker
```

- Rust 1.94+ 와 `cargo` 필요
- 의존하는 [edwardkim/rhwp](https://github.com/edwardkim/rhwp) 는 `Cargo.toml` 의 git dep + rev pin (HOP 가 고정한 `c2e8a3461de800a02f76127ff4797bade1d4e532`) 으로 자동 fetch
- 검증 상태: 8 unit + 8 smoke 테스트 green, 한컴독스에서 생성/수정 파일 시각 확인됨

## 1. 설치 — 다른 머신에서

### 옵션 A: CLI 로 시스템 설치
```sh
cargo install --git https://github.com/0xMegg/hwp-maker --branch main
# 그 뒤로는 어디서든
hwp-maker build --spec my.yaml --out out.hwp
```

### 옵션 B: clone 후 직접 빌드
```sh
git clone https://github.com/0xMegg/hwp-maker ~/work/hwp-maker
cd ~/work/hwp-maker
cargo build --release
./target/release/hwp-maker --help
```

### 옵션 C: Rust 프로젝트의 라이브러리 dep (**더 큰 서비스 빌드용 권장**)
```toml
# Cargo.toml
[dependencies]
hwp-maker = { git = "https://github.com/0xMegg/hwp-maker", branch = "main" }

# 또는 특정 commit/tag 고정 (재현성)
hwp-maker = { git = "https://github.com/0xMegg/hwp-maker", rev = "<commit-sha>" }
```

`hwp_maker::rhwp` 도 같이 re-export 되므로 저수준 API 도 같은 dep 로 통과.

## 2. 라이브러리 API

```rust
use std::path::Path;
use hwp_maker::{build, fill, read_as_json, delete};

// === Create ===
build(Path::new("spec.yaml"), None, Path::new("out.hwp"))?;                          // 템플릿
build(Path::new("spec.yaml"), Some(Path::new("data.yaml")), Path::new("out.hwp"))?;  // 완성본

// === Read ===
let json: String = read_as_json(Path::new("out.hwp"))?;
// 또는 구조체로
let summary: hwp_maker::read_cmd::DocSummary = hwp_maker::read_cmd::dump(Path::new("out.hwp"))?;

// === Update ===
fill(Path::new("tpl.hwp"), Path::new("data.yaml"), Path::new("out.hwp"))?;

// === Delete ===
delete::row(Path::new("in.hwp"), 0 /*table_idx*/, 1 /*row_idx*/, Path::new("out.hwp"))?;
delete::column(Path::new("in.hwp"), 0, 2, Path::new("out.hwp"))?;
delete::table_control(Path::new("in.hwp"), 0, Path::new("out.hwp"))?;
delete::picture_in_cell(Path::new("in.hwp"), 0, 3, Path::new("out.hwp"))?;
```

## 3. CLI

```sh
hwp-maker build   --spec s.yaml [--data d.yaml] --out out.hwp
hwp-maker fill    --template t.hwp --data d.yaml --out out.hwp
hwp-maker read    --file f.hwp                                  # JSON 덤프
hwp-maker inspect --file f.hwp                                  # 한 줄 요약
hwp-maker delete row     --file f.hwp --table 0 --index N --out o.hwp
hwp-maker delete column  --file f.hwp --table 0 --index N --out o.hwp
hwp-maker delete table   --file f.hwp --table 0           --out o.hwp
hwp-maker delete picture --file f.hwp --table 0 --cell N  --out o.hwp
```

## 4. Spec YAML (build 용)

```yaml
version: 1
vars:                                    # {{name}} 텍스트 치환 (placeholder 셀은 예외)
  project_name: "베스트ST"
  date: "2026-04-23"
table:
  border:
    style: solid                         # solid | dashed | dotted | double
    width_pt: 0.5
    color: "#000000"
  cell_padding_pt: 4
  # A4 본문 폭 ≈ 425pt. 컬럼 합이 이를 초과하면 표가 잘림.
  columns:
    - { width_pt: 80 }
    - { width_pt: 160 }
    - { width_pt: 140 }
  rows:
    - { height_pt: 28 }
    - { height_pt: 96 }
    - { height_pt: 160 }
  cells:
    - row: 0
      col: 0
      colspan: 3
      text: "{{project_name}} 현장야장"
      style: { size_pt: 14, weight: bold, align: center, bg: "#EEEEEE" }
    - row: 1
      col: 0
      text: "촬영일자"
      style: { align: center, weight: bold }
    - row: 1
      col: 1
      text: "{{date}}"
      placeholder: shoot_date            # B방식/`--data` 에서 매칭할 키
    - row: 1
      col: 2
      image:
        path: "logo.png"                  # spec 파일 기준 상대 경로
        w_pt: 120
        h_pt: 70
      placeholder: logo
    - row: 2
      col: 1
      colspan: 2
      image: { path: "site.png", w_pt: 280, h_pt: 140 }
```

## 5. Data YAML (fill, build --data 용)

```yaml
version: 1
fields:
  shoot_date: "2026-04-23 14:30"
  logo:
    image: "logo.png"
    w_pt: 120
    h_pt: 70
```

## 6. ★ rhwp 8가지 버그와 워크어라운드 (필독)

rhwp 의 `paste_html_native` 경로는 native export 에서 한컴독스가 거부하는 출력을 만든다. hwp-maker 가 모두 해결했지만, **rhwp 를 직접 쓰려고 하면 모두 다시 재발견하게 됨**.

| # | 버그 | 워크어라운드 위치 |
|---|---|---|
| 1 | 셀 `<img>` 가 plain-text 비어있으면 무시됨 | `src/html/table.rs::render_cell` — `\u{200B}` (ZWSP) 접두 |
| 2 | `parse_img_html` 이 CSS 무시, HTML attr 의 px 만 인식 | 동일 — `width = pt × 4/3` 로 emit |
| 3 | content 가 `<` 로 시작 안 하면 escape 되던 버그 | 동일 — `contains('<')` 로 판별 |
| 4 | Table `raw_ctrl_data` 가 CommonObjAttr 의 첫 `attr` u32 누락 → 표가 Paper 기준 floating, 한컴 거부 | `src/build_cmd.rs::fix_tables_for_hancom_compat` — 42바이트로 재조립, `attr=0x082A2311` |
| 5 | Picture 가 `Picture::default()` → invalid floating | `src/build_cmd.rs::fix_pictures_for_hancom_compat` — `insert_picture_native` 레시피 (`attr=0x0A0211`, `treat_as_char=true`, `vert_rel_to=Para`, `horz_rel_to=Column`, `ShapeComponentAttr` 크기, `crop=natural_px×75`) |
| 6 | Picture paragraph 의 `text="[이미지]"`, `char_count=5`, `control_mask=0` → 이미지 위에 텍스트 justify-spread 됨 | 동일 — `text=""`, `char_count=9`, `control_mask=0x800`, LineSeg 1개; 앞선 ZWSP paragraph 도 제거 |
| 7 | BIN_DATA manifest 가 DocInfo 에 등록 안 됨 → 뷰어가 이미지 참조 못함, 재로드 시 bin_data_content 소실 | `src/build_cmd.rs::register_missing_bin_data` — `doc_info.raw_stream` 에 HWPTAG_BIN_DATA(`attr=0x0001` Embedding) 를 surgical 바이트 삽입, ID_MAPPINGS 의 `bin_data_count` 증가 |
| 8 | ~10MB+ HWP 에서 CFB writer 가 next_id 오류 | 이미지 사이즈/개수 합리적으로 유지 |

## 7. 절대 원칙 (확장·재구현 시 반드시)

1. **`doc_info.raw_stream_dirty = true` 절대 금지.**
   rhwp 의 DocInfo 재직렬화 경로는 한컴독스가 거부함. DocInfo 수정은 항상 `raw_stream` 의 **surgical 바이트 패치**로. 패턴은 `src/build_cmd.rs::register_missing_bin_data` 와 `make_record`/`scan_records`/`find_*` 헬퍼 참고.

2. **`paste_html_native` 호출 후엔 fixup 체인 무조건 실행.**
   1. `fix_tables_for_hancom_compat`
   2. `fix_pictures_for_hancom_compat`
   3. `register_missing_bin_data`

   순서 중요. 빠뜨리면 파일이 안 열리거나 깨져 보임. `build_cmd::run` 이 이 순서로 호출하므로 라이브러리 `build()` 함수를 쓰면 자동 적용.

3. **셀 내부에 Picture 를 신규 삽입하는 공개 API 는 rhwp 에 없다.**
   - `paste_html_in_cell_native` 는 모든 Control 을 strip
   - `insert_picture_native` 는 body paragraph 만 받음
   - `bin_data_content` 는 `pub(crate)`

   "기존 .hwp 에 이미지 새로 넣기" 가 필요하면 **A방식 `build --data` 로 재생성** (또는 `delete::picture_in_cell` 처럼 Document 모델 직접 mutation).

4. **HOP 의 save 가 잘 된다고 rhwp native export 도 잘 되는 건 아님.**
   - HOP 는 `save_hwp_bytes(bytes)` 에서 bytes 가 JS `exportHwp()` (rhwp WASM) 에서 옴.
   - rhwp native `export_hwp_native()` 는 별도 코드 경로이고 위 8개 버그가 있음.

5. **A4 본문 폭 ≈ 425pt.** spec 의 컬럼 합이 이를 초과하면 표가 페이지를 넘어감.

## 8. 더 큰 서비스로 발전시킬 때

hwp-maker 는 라이브러리로 import 해서 그 위에 새 기능을 쌓는 게 의도된 사용법이다. 확장 시 참고할 지점:

### 새 spec 필드 추가
- `src/spec.rs` 에 serde 구조체 필드 추가
- `src/html/table.rs::render_cell` 또는 `src/html/style.rs` 에서 처리
- 반드시 fixup 체인 거치게 유지

### 새 delete/update 연산 추가
- 구조 변경만 (`section.raw_stream = None` 만 invalidate)이면 그대로 export 가능
- 스타일/폰트 변경은 `doc_info.raw_stream_dirty = true` 를 유발하므로 **별도 surgical 패치 헬퍼** 필요. 절대 원칙 #1 참고.
- 패턴: `src/delete_cmd.rs::picture_in_cell` 처럼 `core.document().clone()` → mutate → `core.set_document(doc)`

### 새 입력 포맷 (HWPX 등)
- rhwp 가 `DocumentCore::from_bytes` 에서 자동 감지 (`source_format: Hwp | Hwpx`)
- 저장은 `export_hwp_native` (HWP) 또는 `export_hwpx_native` (HWPX). 후자는 rhwp 자체 미완성 영역이라 더 조심.

### 새 CRUD 백엔드 (웹 API 등)
- `src/lib.rs` 의 4개 entry function (`build`, `fill`, `read_as_json`, `delete::*`) 을 그대로 wrapper 함수에서 호출
- 각각 input/output 이 `&Path` 라서 임시 디렉터리 + multipart 업로드 형태로 wrap 하기 쉬움

## 9. 다른 Claude 세션 / 머신에서 활용

### 옵션 A: 컨텍스트 한 줄로 가이드 위치만 알려주기
```
hwp 파일 다뤄야 함. https://github.com/0xMegg/hwp-maker — docs/PORTABLE_GUIDE.md 읽고 진행해.
8개 rhwp 버그 워크어라운드가 이미 다 들어있으니 재구현하지 말고 라이브러리로 import 해서 써.
```

### 옵션 B: 새 프로젝트의 Claude 메모리에 영구 등록
```sh
# 새 작업 폴더의 슬러그 (cwd 의 / 를 - 로)
NEW_SLUG="-Users-mero-projects-foo"   # 예시

mkdir -p ~/.claude/projects/${NEW_SLUG}/memory/

# 가이드 파일을 복사하거나, git 으로 가져오기
curl -fsSL https://raw.githubusercontent.com/0xMegg/hwp-maker/main/docs/PORTABLE_GUIDE.md \
  > ~/.claude/projects/${NEW_SLUG}/memory/reference_hwp_crud.md

# MEMORY.md 에 인덱스 추가
cat >> ~/.claude/projects/${NEW_SLUG}/memory/MEMORY.md << 'EOF'
- [HWP CRUD via rhwp](reference_hwp_crud.md) — github.com/0xMegg/hwp-maker, 8개 rhwp 버그·워크어라운드, CRUD 예제
EOF
```

### 옵션 C: Cargo dep (Rust 프로젝트인 경우)
```toml
[dependencies]
hwp-maker = { git = "https://github.com/0xMegg/hwp-maker", branch = "main" }
```

## 10. 트러블슈팅

| 증상 | 원인 | 해결 |
|---|---|---|
| 파일이 한컴독스에서 "문서를 열 수 없습니다" | fixup 체인 안 돔 / `raw_stream_dirty=true` 발생 | `build_cmd::run` 사용 확인 / 직접 짠 경로면 워크어라운드 3개 모두 적용 |
| 이미지가 안 보임 | BIN_DATA manifest 미등록 (#7) | `register_missing_bin_data` 호출 확인 |
| 이미지 위에 "[이미지]" 가 큰 글자로 퍼짐 | Picture paragraph 구조 미수정 (#6) | `fix_picture_paragraph` 호출 확인 |
| 표가 페이지 오른쪽으로 밀려 잘림 | Table CommonObjAttr `attr` 누락 (#4) 또는 컬럼 합 > 425pt | fixup 적용 + 컬럼 폭 합 줄이기 |
| 셀 안의 이미지가 사라짐 | `parse_table_html` 의 빈 plain-text 체크에 걸림 (#1) | `<img>` 앞에 `\u{200B}` 접두 |
| 이미지 크기가 spec 과 다름 | CSS px 단위로 인식됨 (#2) | HTML `width="..."` attr 로 `pt × 4/3` 값 전달 |
| 대용량 HWP 가 재파싱 실패 | rhwp CFB writer 한계 (#8) | 이미지 압축/축소 |
| B방식 fill 이 이미지 무시 | rhwp 공개 API 가 셀 Picture 삽입 미지원 | A방식 `build --data` 로 재생성 |

## 11. rhwp 소스 위치 (디버깅용)

git dep 가 fetch 한 위치:
```
~/.cargo/git/checkouts/rhwp-*/c2e8a34/src/
  document_core/commands/object_ops.rs:1098    # insert_picture_native — 올바른 Picture 레시피
  document_core/html_table_import.rs           # parse_img_html, parse_table_html — 버그 발원지
  document_core/commands/html_import.rs        # paste_html_native, paste_html_in_cell_native
  parser/control/shape.rs:247                  # parse_common_obj_attr (Table/Picture attr 파싱)
  serializer/control.rs:345                    # serialize_table (raw_ctrl_data 그대로 emit)
  serializer/doc_info.rs:22                    # serialize_doc_info (raw_stream_dirty 체크)
```
