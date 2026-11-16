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

#[test]
fn streaming_an_item_after_a_blank_line_matches_a_full_parse() {
    // "1" is a paragraph of its own until its "." makes it the list's next item.
    assert_streams_like_full_parse("1. first\n\n1. second\n\n1. third");
}

#[test]
fn streaming_a_lazy_line_after_a_loose_list_matches_a_full_parse() {
    // "---" is a rule until its "|" makes it a lazy line of the nested item,
    // in a list with a blank line inside and none at its end.
    assert_streams_like_full_parse("- item\npara *two\n\n  - nested\n---|");
}

/// How deep the quotes and lists of `blocks` nest.
fn nesting(blocks: &[Block]) -> usize {
    let inside = |block: &Block| match block {
        Block::Quote(blocks) => 1 + nesting(blocks),
        Block::List { items, .. } => {
            let items = items.iter().map(|item| nesting(&item.blocks));
            1 + items.max().unwrap_or(0)
        }
        _ => 0,
    };
    blocks.iter().map(inside).max().unwrap_or(0)
}

#[test]
fn quotes_and_lists_nest_no_deeper_than_the_limit() {
    use super::parse::MAX_NESTING;

    for marker in [">", "- ", "1. ", "> - "] {
        let parsed = blocks(&format!("{}deep", marker.repeat(2000)));
        assert_eq!(nesting(&parsed), MAX_NESTING, "{marker:?}");
    }
    // At the limit and one past it, the text is in the deepest quote kept.
    for levels in [MAX_NESTING, MAX_NESTING + 1] {
        let parsed = blocks(&format!("{} deep", ">".repeat(levels)));
        let mut inside = parsed.as_slice();
        for level in 0..MAX_NESTING {
            let [Block::Quote(blocks)] = inside else {
                panic!("a quote at level {level} of {levels}: {inside:?}");
            };
            inside = blocks;
        }
        assert_eq!(inside.iter().map(plain).collect::<Vec<_>>(), ["deep"]);
    }
}

#[test]
fn a_list_nested_past_the_limit_keeps_its_text_in_the_last_item_kept() {
    use super::parse::MAX_NESTING;

    // One item a level, each holding the list of the next, three past the
    // limit, and a quote and a task in the deepest.
    let levels = MAX_NESTING + 3;
    let mut source: String = (0..levels)
        .map(|level| format!("{}- item {level}\n", "  ".repeat(level)))
        .collect();
    source += &format!("{0}- [x] task\n{0}- > quoted\n", "  ".repeat(levels));
    source += "- after\n";
    let parsed = blocks(&source);
    assert_eq!(nesting(&parsed), MAX_NESTING);

    let [Block::List { items: top, .. }] = parsed.as_slice() else {
        panic!("one list: {parsed:?}");
    };
    assert_eq!(top.len(), 2, "the list goes on after what was nested");
    assert_eq!(plain(&top[1].blocks[0]), "after");
    let mut item = &top[0];
    for level in 0..MAX_NESTING - 1 {
        assert_eq!(plain(&item.blocks[0]), format!("item {level}"));
        let [_, Block::List { items, .. }] = item.blocks.as_slice() else {
            panic!("level {level} holds a list: {:?}", item.blocks);
        };
        item = &items[0];
    }
    assert_eq!(item.task, None, "a task nested deeper is not this item's");
    let kept: Vec<_> = item.blocks.iter().map(plain).collect();
    let last = MAX_NESTING - 1;
    assert_eq!(
        kept,
        [
            format!("item {last}"),
            format!("item {}", last + 1),
            format!("item {}", last + 2),
            format!("item {}", last + 3),
            "task".to_string(),
            "quoted".to_string(),
        ]
    );
}
