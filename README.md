# hwp-maker

HWP 문서 CRUD CLI. [edwardkim/rhwp](https://github.com/edwardkim/rhwp) 엔진을 Rust crate 로 직접 링크해서 프로그래밍적으로 HWP 표를 만들고 읽고 수정하고 삭제한다.

## 무엇이 되는가

| 작업 | 명령 | 이미지 | 텍스트 | 사이즈 |
|---|---|---|---|---|
| **Create (A방식 template)** | `build --spec` | ✅ | ✅ | ✅ pt 정확 |
| **Create (A방식 fully rendered)** | `build --spec --data` | ✅ | ✅ | ✅ pt 정확 |
| **Update (B방식 fill)** | `fill --template --data` | ⚠️ 불가, `build --data` 사용 | ✅ | ✅ 보존 |
| **Read** | `read --file` | — JSON 덤프 (cells, BinData, page count) | | |
| **Delete (row/col/table/picture)** | `delete <sub>` | 구조적 삭제 (rhwp 네이티브) | | |

## 빠른 시작

```bash
cargo build

# ===== Create =====
# A방식: 템플릿 생성 (placeholder 셀은 {{...}} 텍스트로)
cargo run --quiet -- build --spec examples/minimal_table.yaml --out out/template.hwp

# A방식: 데이터까지 적용한 완성본
cargo run --quiet -- build --spec examples/minimal_table.yaml --data examples/fill_data.yaml --out out/filled.hwp

# ===== Update =====
# B방식: 기존 템플릿 .hwp의 텍스트 placeholder 치환 (이미지는 보존되지만 추가/교체 불가)
cargo run --quiet -- fill --template out/template.hwp --data examples/fill_data.yaml --out out/filled_via_fill.hwp

# ===== Read =====
# JSON 구조 덤프 (sections / paragraphs / tables / cells / BinData manifest)
cargo run --quiet -- read --file out/template.hwp

# 간단 인스펙션
cargo run --quiet -- inspect --file out/template.hwp

# ===== Delete =====
# 표의 1번째 행 삭제
cargo run --quiet -- delete row --file out/template.hwp --table 0 --index 1 --out out/shrunk.hwp

# 표의 2번째 열 삭제
cargo run --quiet -- delete column --file out/template.hwp --table 0 --index 2 --out out/shrunk.hwp

# 표 자체를 통째로 삭제
cargo run --quiet -- delete table --file out/template.hwp --table 0 --out out/no_table.hwp

# 특정 셀의 그림만 삭제 (셀 인덱스는 row-major 평탄화, `read` 로 확인)
cargo run --quiet -- delete picture --file out/template.hwp --table 0 --cell 3 --out out/no_pic.hwp
```

## 라이브러리로 사용

다른 Rust crate 에서 `hwp-maker` 를 직접 호출할 수 있다. `src/lib.rs` 가 4개의 최상위 함수를 re-export 한다.

```toml
[dependencies]
hwp-maker = { path = "/Users/mero/Dev/13.claude/workouts/beststcad/hwp-maker" }
```

```rust
use std::path::Path;
use hwp_maker::{build, fill, read_as_json, delete};

// Create
build(Path::new("spec.yaml"), None, Path::new("out.hwp"))?;
build(Path::new("spec.yaml"), Some(Path::new("data.yaml")), Path::new("filled.hwp"))?;

// Update
fill(Path::new("template.hwp"), Path::new("data.yaml"), Path::new("filled.hwp"))?;

// Read
let json = read_as_json(Path::new("filled.hwp"))?;

// Delete
delete::row(Path::new("in.hwp"), 0 /*table idx*/, 1 /*row idx*/, Path::new("out.hwp"))?;
delete::column(Path::new("in.hwp"), 0, 2, Path::new("out.hwp"))?;
delete::table_control(Path::new("in.hwp"), 0, Path::new("out.hwp"))?;
delete::picture_in_cell(Path::new("in.hwp"), 0, 3, Path::new("out.hwp"))?;
```

rhwp 자체도 `hwp_maker::rhwp` 로 re-export 되어 있어 저수준 API 도 같이 쓸 수 있다.

## Spec YAML

```yaml
version: 1
vars:
  project_name: "베스트ST"
  date: "2026-04-23"
table:
  border: { style: solid, width_pt: 0.5, color: "#000000" }
  cell_padding_pt: 4
  columns:
    - { width_pt: 120 }
    - { width_pt: 260 }
    - { width_pt: 160 }
  rows:
    - { height_pt: 28 }
    - { height_pt: 96 }
  cells:
    # 제목 행 (병합)
    - row: 0
      col: 0
      colspan: 3
      text: "{{project_name}} 현장야장"
      style: { size_pt: 14, weight: bold, align: center, bg: "#EEEEEE" }

    # 일반 레이블 셀
    - row: 1
      col: 0
      text: "촬영일자"
      style: { align: center, weight: bold }

    # 텍스트 placeholder 셀 (B방식에서 {{shoot_date}} 로 치환)
    - row: 1
      col: 1
      text: "{{date}}"
      placeholder: shoot_date

    # 이미지 placeholder 셀 (B방식에서 {{image:logo}} 마커)
    - row: 1
      col: 2
      image:
        path: "fixtures/site.png"
        w_pt: 140
        h_pt: 80
      placeholder: logo
```

## Data YAML (build --data / fill)

```yaml
version: 1
fields:
  shoot_date: "2026-04-23 14:30"
  logo:
    image: "fixtures/site.png"
    w_pt: 140
    h_pt: 80
```

## 기술 검증 결과

### A방식: 완전 지원
- 빈 HWP 문서 생성 → HTML+CSS 기반 표 삽입 → 저장 → 재파싱 모두 동작
- **셀 사이즈 정확도**: pt 단위가 그대로 HWPUNIT으로 매핑됨 (`rhwp` 의 `parse_css_dimension_pt` + `px_to_hwpunit` 경로)
- **이미지 임베드**: HTML `<img src="data:...;base64,...">` 태그가 `parse_img_html` 로 HWP `BinData` 에 등록되고 셀 안 Picture control 로 들어감

### rhwp 버그 · 워크어라운드 목록

한컴독스(Hancom Docs)에서 실제로 파일이 열리고 정상 렌더링되기까지 발견된 rhwp의 버그들과 우리 쪽 워크어라운드:

1. **셀 내부 `<img>` 파싱 조건**: rhwp의 `parse_table_html` 은 각 셀 `content_html` 의 `html_to_plain_text()` 결과가 비어있으면 `<img>` 를 처리하지 않고 빈 셀로 둔다. 단독 `<img>` 셀은 plain-text 가 비므로 이미지가 사라진다. → `<img>` 앞에 **U+200B (ZERO WIDTH SPACE)** 를 넣어 plain-text 가 비지 않게 만든다. (`src/html/table.rs`)

2. **이미지 크기 단위**: `parse_img_html` 은 CSS `style` 이 아니라 HTML **속성** `width="..."` / `height="..."` 에서만 크기를 읽고, 값은 **px** 로 취급한다 (`px_to_hwpunit(px, 96dpi) = px × 75`). → spec의 `w_pt` 를 `px = pt × 4/3` 로 변환해서 `width` 속성에 emit. 결과: `pt × 100 = HWPUNIT` 정확 매핑.

3. **HTML 이스케이프**: 셀 content 가 `<` 로 시작하지 않으면 (예: ZWSP 접두) 전체를 `html_escape` 하던 버그. `content.contains('<')` 로 판별. (`src/html/table.rs`)

4. **Table CommonObjAttr 누락** (한컴 파일 로드 실패의 주원인 ①): rhwp의 `parse_table_html` 이 만드는 `table.raw_ctrl_data` 는 `CommonObjAttr` 포맷에서 **선두 `attr` u32(4바이트)를 빠뜨리고** 나머지 필드를 그대로 넣는다. 파서 `parse_common_obj_attr` 는 `attr` 을 먼저 읽으므로 모든 필드가 4바이트씩 shift되어 표가 `attr=0`(Paper 기준 absolute 배치)으로 해석된다. 한컴독스는 이 상태의 표를 거부. → `fix_tables_for_hancom_compat()` 에서 `raw_ctrl_data` 를 완전히 재조립 (`attr=0x082A2311 | treat_as_char | Para | Column` + 나머지 필드) 하고 `table.common` 도 동기화. (`src/build_cmd.rs`)

5. **Picture CommonObjAttr 누락** (한컴 로드 실패의 주원인 ②): rhwp의 `parse_img_html` 은 `Picture::default()` 만 세팅해서 `treat_as_char=false`, `vert_rel_to=Paper`, anchor 없음으로 Picture가 invalid한 floating object가 된다. → `fix_pictures_for_hancom_compat()` 에서 `object_ops.rs:1098` 의 `insert_picture_native` 레시피를 그대로 적용: `ctrl_id="gso "`, `attr=0x0A0211`, `treat_as_char=true`, `vert_rel_to=Para`, `horz_rel_to=Column`, `ShapeComponentAttr` 크기/버전, border 4모서리, `crop=이미지 원본px × 75`. 이미지 원본 크기는 PNG/JPEG 헤더를 직접 파싱해서 추출.

6. **Picture paragraph 구조 누락**: `parse_img_html` 은 Picture 컨트롤을 `text: "[이미지]"`, `char_count: 5`, `control_mask: 0` 인 paragraph에 넣는다. 올바른 inline Picture paragraph는 `text: ""`, `char_count: 9` (확장 제어문자 8 + 문단끝 1), `control_mask: 0x0800`, LineSeg 1개 여야 한다. 그렇지 않으면 "[이미지]" 텍스트가 justify-spread 되어 이미지 위에 표시된다. → 동일 함수에서 paragraph 전체를 `insert_picture_native` 의 `pic_para` 구조로 재조립하고, 앞선 ZWSP 더미 paragraph도 정리.

7. **BIN_DATA manifest 누락**: `parse_img_html` 이 이미지 바이트를 `document.bin_data_content` 에만 추가하고, DocInfo 의 `bin_data_list` (HWP 뷰어가 BIN 스트림을 찾기 위한 manifest) 에는 등록하지 않는다. 그 결과 저장된 HWP 파일은 `/BinData/BIN0001.png` 스트림은 있지만 DocInfo 는 비어있어 **뷰어가 Picture의 bin_data_id 참조를 해결 못함**. → `register_missing_bin_data()` 가 `doc_info.raw_stream` 을 surgical byte patch 하여 `HWPTAG_BIN_DATA` 레코드(`attr=0x0001` Embedding + storage_id + extension)를 ID_MAPPINGS 직후에 삽입하고 `bin_data_count` (ID_MAPPINGS 첫 u32)를 증가. `raw_stream_dirty` 는 건드리지 않아 원본 DocInfo 바이트는 그대로 보존.

8. **대용량 직렬화 한계**: 5.6MB PNG 이미지 3장 이상을 한 문서에 넣으면 `export_hwp_native()` 는 성공하지만 재파싱이 CFB next_id 오류로 실패한다. → 이미지 사이즈/개수를 합리적으로 유지.

9. **페이지 본문 폭**: A4 기본 여백으로 컨텐츠 영역은 약 **425pt**. spec의 컬럼 폭 합이 이를 초과하면 표가 잘려 보인다. → YAML 작성 시 `columns[].width_pt` 합 < 425 유지.

### B방식: 텍스트 치환 지원, 이미지 치환 불가
- **지원**: `{{key}}` 패턴 셀을 찾아 `delete_text_in_cell_native` + `insert_text_in_cell_native` 로 치환. 셀 사이즈와 서식이 그대로 보존된다.
- **미지원**: `{{image:key}}` 셀에 Picture control 을 새로 삽입하는 경로가 rhwp public API 에 없다.
  - `paste_html_in_cell_native` 는 셀 내부 control 을 모두 **strip** 한다 (rhwp `src/document_core/commands/html_import.rs:200-226`, 주석: "셀 내부에는 Table Control 중첩 불가 → 컨트롤 포함 문단은 텍스트만 추출"). Picture도 같이 떨어져 나간다.
  - `insert_picture_native` / `set_picture_properties_native` 는 body paragraph 주소 `(section, para, ctrl)` 만 받고 셀 내부에는 접근 불가.
  - `BinData` (이미지 바이트) 교체는 `document.bin_data_content` 가 `pub(crate)` 라 외부에서 불가.
  - **해결 경로**: 이미지를 갱신해야 하는 경우는 `fill` 대신 **`build --spec <orig.yaml> --data <new-data.yaml>`** 로 A방식 재생성을 쓴다. 템플릿 설계가 사양(YAML) 기반이라 이게 자연스러운 흐름이다.

### 치수 정합성 (rhwp 내부 수식)
- `1pt = 100 HWPUNIT` (1/72 inch × 7200 HWPUNIT/inch = 100)
- 이미지: `px = pt × 4/3` (96dpi 관례) 로 HTML `width`/`height` 에 emit → `px_to_hwpunit(px, 96) = px × 75` → HWPUNIT. 회귀 오차는 반올림뿐.
- CSS 단위: `pt` 그대로, `px × 0.75`, `cm × 28.3465`, `mm × 2.83465`, `in × 72.0`. `%` 는 0 으로 무시됨.

## 프로젝트 구조

```
src/
  lib.rs           # 퍼블릭 re-export (build / fill / read_as_json / delete::*)
  cli.rs           # clap 서브커맨드 (Build / Fill / Read / Inspect / Delete)
  error.rs         # AppError + rhwp error mapping
  spec.rs          # A방식 YAML 모델
  data.rs          # B방식 YAML 모델
  vars.rs          # {{name}} 변수 치환 (placeholder 셀은 예외)
  placeholder.rs   # {{key}} / {{image:key}} 파서
  html/
    mod.rs
    style.rs       # 셀/텍스트 inline CSS 생성
    table.rs       # <table> HTML 빌더 (ZWSP trick 포함)
    image.rs       # 파일 → base64 data URL
  traverse.rs      # 공개 rhwp API 로만 셀 순회 (probe-based)
  build_cmd.rs     # A방식 Create + rhwp 버그 8개 워크어라운드
  fill_cmd.rs      # B방식 Update (텍스트 치환)
  read_cmd.rs      # Read: serde 기반 JSON 구조 덤프
  delete_cmd.rs    # Delete: row/col/table/picture-in-cell
  main.rs          # CLI 엔트리
examples/
  minimal_table.yaml
  text_only.yaml
  fill_data.yaml
  fixtures/
    site_small.png
tests/
  smoke.rs         # end-to-end 스모크 테스트 (8종)
```

## 테스트

```bash
cargo test
```

현재 8개 unit 테스트 + 4개 smoke 테스트 통과.

## 한계와 향후 과제

- **B방식 이미지 치환**: rhwp 업스트림에 `insert_picture_in_cell_native` 또는 `set_cell_picture_data_native` 가 추가되기 전까지는 **A방식 `build --data`로 재생성**하는 게 우회 경로. 사용자 관점에서는 동일한 YAML 템플릿 + 데이터 YAML 조합.
- **다문단 셀 내 placeholder**: 첫 번째 문단만 검사. 복잡한 레이아웃은 단일 placeholder 셀로 단순화 권장.
- **대용량 HWP 직렬화**: ~10MB 이상에서 rhwp CFB writer 가 불안정. 이미지 압축/축소 권장.
- **폰트 시스템**: CSS `font-family` 는 emit 하지만 시스템 폰트 매핑 검증은 수동.
- **rhwp 버그의 근본적 해결**: 현재 8개의 워크어라운드 중 상당수는 rhwp 에 PR 을 보내 고쳐야 할 진짜 버그들이다 (특히 #4, #5, #6, #7). 시간이 나면 업스트림 PR 제안 고려.

## 검증된 워크플로

현재 한컴독스에서 열기 · 렌더링이 확인된 시나리오:

```bash
# A방식: 템플릿 생성 (B방식 입력으로 재사용 가능)
cargo run -- build --spec examples/minimal_table.yaml --out out/template.hwp

# A방식: 데이터 적용한 완성본 (이미지 포함 시 권장 경로)
cargo run -- build --spec examples/minimal_table.yaml --data examples/fill_data.yaml --out out/filled.hwp

# B방식: 기존 템플릿에 텍스트 치환 (이미지는 템플릿의 것 유지)
cargo run -- fill --template out/template.hwp --data examples/fill_data.yaml --out out/filled.hwp
```

모두 한컴독스(Hancom Docs) 에서 정상적으로 열리고 spec에 명시한 pt 크기가 그대로 반영됨.

## 의존성

- [rhwp](https://github.com/edwardkim/rhwp) `@c2e8a34...` (HOP 가 고정한 커밋). crates.io 미발행이라 git dep.
- clap, serde/serde_yaml/serde_json, anyhow/thiserror, base64, image

## License

MIT
