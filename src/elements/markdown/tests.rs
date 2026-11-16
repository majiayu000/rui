use super::parse::{Block, Document, parse};

fn blocks(source: &str) -> Vec<Block> {
    parse(source)
        .blocks
        .iter()
        .map(|(_, block)| (**block).clone())
        .collect()
}

fn plain(block: &Block) -> &str {
    match block {
        Block::Paragraph(inline) | Block::Heading(_, inline) => &inline.text,
        other => panic!("{other:?} is not a paragraph"),
    }
}

#[test]
fn inline_styles_become_runs_of_one_paragraph() {
    let parsed = blocks("a **b** *c* ~~d~~ `e` [f](g)");
    let [Block::Paragraph(inline)] = parsed.as_slice() else {
        panic!("one paragraph: {parsed:?}");
    };
    assert_eq!(&*inline.text, "a b c d e f");
    let styled: Vec<_> = inline
        .runs
        .iter()
        .map(|run| {
            (
                &inline.text[run.bytes.clone()],
                run.style,
                run.link.as_deref(),
            )
        })
        .collect();
    assert!(
        styled
            .iter()
            .any(|(text, style, _)| *text == "b" && style.strong)
    );
    assert!(
        styled
            .iter()
            .any(|(text, style, _)| *text == "c" && style.emphasis)
    );
    assert!(
        styled
            .iter()
            .any(|(text, style, _)| *text == "d" && style.strikethrough)
    );
    assert!(
        styled
            .iter()
            .any(|(text, style, _)| *text == "e" && style.code)
    );
    assert!(
        styled
            .iter()
            .any(|(text, _, link)| *text == "f" && *link == Some("g"))
    );
    let covered: usize = inline.runs.iter().map(|run| run.bytes.len()).sum();
    assert_eq!(covered, inline.text.len());
}

#[test]
fn code_blocks_keep_their_indentation() {
    let parsed = blocks("```\nfn main() {\n    body();\n}\n```\n\n    indented\n");
    assert_eq!(
        parsed,
        [
            Block::Code("fn main() {\n    body();\n}".into()),
            Block::Code("indented".into())
        ]
    );
}

#[test]
fn lists_nest_and_carry_task_markers() {
    let parsed = blocks("3. three\n   - inner\n\n- [x] done\n- [ ] open\n");
    let [
        Block::List {
            start: Some(3),
            items: ordered,
        },
        Block::List {
            start: None,
            items: tasks,
        },
    ] = parsed.as_slice()
    else {
        panic!("an ordered list then a task list: {parsed:?}");
    };
    assert_eq!(plain(&ordered[0].blocks[0]), "three");
    assert!(matches!(
        ordered[0].blocks[1],
        Block::List { start: None, .. }
    ));
    assert_eq!(
        tasks.iter().map(|item| item.task).collect::<Vec<_>>(),
        [Some(true), Some(false)]
    );
}

#[test]
fn tables_quotes_rules_and_images() {
    let parsed = blocks(
        "> quoted\n\n---\n\n| a | b |\n|---|--:|\n| 1 | 2 |\n\ntext ![alt](pic.png) after\n",
    );
    assert!(matches!(&parsed[0], Block::Quote(inner) if plain(&inner[0]) == "quoted"));
    assert_eq!(parsed[1], Block::Rule);
    let Block::Table { head, rows, .. } = &parsed[2] else {
        panic!("a table: {:?}", parsed[2]);
    };
    assert_eq!(&*head[1].text, "b");
    assert_eq!(&*rows[0][0].text, "1");
    // An image splits its paragraph.
    assert_eq!(plain(&parsed[3]), "text ");
    assert!(matches!(&parsed[4], Block::Image { url, alt } if &**url == "pic.png" && alt == "alt"));
    assert_eq!(plain(&parsed[5]), " after");
}

/// Every prefix of `source`, extended token by token, parses to what a full
/// parse of it gives.
fn assert_streams_like_full_parse(source: &str) {
    let mut previous = Document::default();
    let mut end = 0;
    for (index, _) in source.char_indices().skip(1).chain([(source.len(), ' ')]) {
        let prefix = &source[..index];
        let extended = super::parse::extend(&previous, prefix).unwrap_or_else(|| parse(prefix));
        let full = parse(prefix);
        let flatten = |document: &Document| {
            document
                .blocks
                .iter()
                .map(|(start, block)| (*start, (**block).clone()))
                .collect::<Vec<_>>()
        };
        assert_eq!(flatten(&extended), flatten(&full), "after {prefix:?}");
        previous = extended;
        end = index;
    }
    assert_eq!(end, source.len());
}

#[test]
fn streaming_matches_a_full_parse_at_every_prefix() {
    assert_streams_like_full_parse(
        "# Title\n\nSome *em* text\nthat continues.\n\n- one\n- two\n  - nested\n\n1. first\n\n\
         > quote\n> more\n\n```rust\nlet x = 1;\n```\n\n    indented\n\nSetext\n---\n\n\
         | a | b |\n|---|---|\n| 1 | 2 |\nafter table\n\n***\n\n> - quoted list\n>   more\n\ntext ![alt](p.png) after [l](u)\n<div>\nhtml\n</div>\n\nend",
    );
}

#[test]
fn reference_definitions_force_a_full_parse() {
    let previous = parse("[a][r]\n\nmore");
    assert!(super::parse::extend(&previous, "[a][r]\n\nmore\n\n[r]: /x").is_none());
}

#[test]
fn streaming_an_indented_paragraph_an_image_splits_matches_a_full_parse() {
    // The blocks an image splits a paragraph into all start past its indent.
    assert_streams_like_full_parse("intro\n\n   see ![cat](missing.png) and more\n\nend");
    assert_streams_like_full_parse("  continued\ntext ![i](u) more");
}
