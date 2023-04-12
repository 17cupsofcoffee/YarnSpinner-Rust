use std::fmt::Display;
use winnow::{
    bytes::{tag, take_till1},
    character::{alpha1, line_ending, space0},
    error::Error,
    multi::{many0, many1},
    prelude::*,
    sequence::{delimited, preceded, separated_pair, terminated},
    stream::AsChar,
};

// See https://github.com/YarnSpinnerTool/YarnSpinner/blob/v2.3.0/YarnSpinner.Compiler/YarnSpinnerParser.g4
pub fn parse(input: &str) -> (Vec<FileHashtag>, Dialogue) {
    parse_dialogue(input).finish().unwrap()
}

/*
    dialogue
        : (file_hashtag*) node+
        ;
*/
fn parse_dialogue(input: &str) -> IResult<&str, (Vec<FileHashtag>, Dialogue)> {
    ((parse_file_hashtags, many1(parse_node)))
        .map(|(hashtags, nodes)| (hashtags, Dialogue { nodes }))
        .parse_next(input)
}

fn parse_file_hashtags(input: &str) -> IResult<&str, Vec<FileHashtag>> {
    many0(parse_file_hashtag).parse_next(input)
}

#[test]
fn parse_file_hashtags_test() {
    let (rest, file_hashtags) = parse_file_hashtags("#abc def \n# abc def ghi \nafter").unwrap();

    assert_eq!(file_hashtags[0].hashtag_text, "abc def ");
    assert_eq!(file_hashtags[1].hashtag_text, " abc def ghi ");
    assert_eq!(rest, "after");
}

/*
   file_hashtag
       : HASHTAG text=HASHTAG_TEXT
       ;
*/
fn parse_file_hashtag(input: &str) -> IResult<&str, FileHashtag> {
    ("#", hashtag_text)
        .map(|(_, text)| FileHashtag { hashtag_text: text })
        .context("File Hashtag")
        .parse_next(input)
}

#[test]
fn parse_file_hashtag_test() {
    let (rest, file_hashtag) = parse_file_hashtag("#abc def \nafter").unwrap();
    assert_eq!(file_hashtag.hashtag_text, "abc def ");
    assert_eq!(rest, "after");
}

// TODO: forbid those as in g4 of reference or not needed as we do lex/parse in one step?
/* ~[ \t\r\n#$<]+ */
fn hashtag_text(input: &str) -> IResult<&str, &str> {
    terminated(take_till1(|x| x == '\r' || x == '\n'), line_ending)
        .context("Hashtag Text")
        .parse_next(input)
}

#[test]
fn hashtag_text_test() {
    let (rest, hashtag_text) = hashtag_text("abc def \nafter").unwrap();
    assert_eq!(hashtag_text, "abc def ");
    assert_eq!(rest, "after");
}

// Remark: Every node must have the title header, but that isn't verfied here, all that's done is ensuring at least one header ist present.
/*
   node
       : header+  BODY_START  body BODY_END
       ;
*/
fn parse_node(input: &str) -> IResult<&str, Node> {
    (many1(parse_header), parse_delimited_body)
        .map(|(headers, body)| Node { headers, body })
        .context("Node")
        .parse_next(input)
}

/*
    node
       : header+  BODY_START  body BODY_END
       ;

   body
       : statement*
       ;
*/
fn parse_delimited_body(input: &str) -> IResult<&str, Vec<Statement>> {
    delimited(
        parse_body_start_marker,
        many0(parse_statement),
        parse_body_end_marker,
    )
    .context("Delimited body")
    .parse_next(input)
}

fn parse_statement(input: &str) -> IResult<&str, Statement> {
    terminated(take_till1(|x| x == '\r' || x == '\n'), line_ending)
        .map(|text| Statement {
            line_statement: text,
        })
        .parse_next(input)
}

fn parse_body_start_marker(input: &str) -> IResult<&str, ()> {
    terminated(tag("---"), line_ending)
        .map(|_| ())
        .parse_next(input)
}

fn parse_body_end_marker(input: &str) -> IResult<&str, ()> {
    terminated(tag("==="), line_ending)
        .map(|_| ())
        .parse_next(input)
}

/*
   header
       : header_key=ID HEADER_DELIMITER  header_value=REST_OF_LINE?
       ;
*/
fn parse_header(input: &str) -> IResult<&str, Header> {
    terminated(
        separated_pair(
            parse_identifier,
            parse_header_delimiter,
            take_till1(|x| x == '\r' || x == '\n'),
        ),
        line_ending,
    )
    .map(|(header_key, header_value)| Header {
        header_key,
        header_value,
    })
    .parse_next(input)
}

// TODO: allow underscore as well?
fn parse_identifier(input: &str) -> IResult<&str, &str> {
    alpha1
        .verify(|id: &str| id.chars().nth(0).map_or(false, AsChar::is_alpha))
        .context("Identifier")
        .parse_next(input)
}

fn parse_header_delimiter(input: &str) -> IResult<&str, &str> {
    preceded(":", space0).parse_next(input)
}

#[derive(Debug)]
pub struct Dialogue<'a> {
    nodes: Vec<Node<'a>>,
}

#[derive(Debug)]
pub struct FileHashtag<'a> {
    hashtag_text: &'a str,
}

#[derive(Debug)]
pub struct Node<'a> {
    headers: Vec<Header<'a>>,
    body: Vec<Statement<'a>>,
}

/*
   statement
       : line_statement
       | if_statement
       | set_statement
       | shortcut_option_statement
       | call_statement
       | command_statement
       | declare_statement
       | jump_statement
       | INDENT statement* DEDENT
       ;
*/
#[derive(Debug)]
pub struct Statement<'a> {
    // TODO: all variants
    line_statement: &'a str,
}

#[derive(Debug)]
pub struct Header<'a> {
    header_key: &'a str,
    header_value: &'a str,
}

impl<'a> Display for Header<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.header_key, self.header_value)
    }
}

impl<'a> Display for Statement<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.line_statement)
    }
}

impl<'a> Display for Dialogue<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.nodes)
    }
}

impl<'a> Display for Node<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {:?}", self.headers, self.body)
    }
}

impl<'a> Display for FileHashtag<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "# {}", self.hashtag_text)
    }
}
