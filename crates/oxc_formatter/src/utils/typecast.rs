use oxc_ast::{
    Comment,
    ast::{ArrowFunctionExpression, Function},
};
use oxc_span::GetSpan;

use crate::{
    Buffer, Format, FormatResult, format_args,
    formatter::{Formatter, prelude::*, trivia::FormatLeadingComments},
    generated::ast_nodes::AstNode,
    write,
    write::{
        FormatFunctionOptions, FormatJsArrowFunctionExpression,
        FormatJsArrowFunctionExpressionOptions,
    },
};

impl<'a> Format<'a, FormatJsArrowFunctionExpressionOptions>
    for &AstNode<'a, ArrowFunctionExpression<'a>>
{
    fn fmt(&self, f: &mut Formatter<'_, 'a>) -> FormatResult<()> {
        AstNode::<ArrowFunctionExpression>::fmt(self, f)
    }

    fn fmt_with_options(
        &self,
        options: FormatJsArrowFunctionExpressionOptions,
        f: &mut Formatter<'_, 'a>,
    ) -> FormatResult<()> {
        AstNode::<ArrowFunctionExpression>::fmt_with_options(self, options, f)
    }
}

impl<'a> Format<'a, FormatFunctionOptions> for &AstNode<'a, Function<'a>> {
    fn fmt(&self, f: &mut Formatter<'_, 'a>) -> FormatResult<()> {
        AstNode::<Function>::fmt(self, f)
    }

    fn fmt_with_options(
        &self,
        options: FormatFunctionOptions,
        f: &mut Formatter<'_, 'a>,
    ) -> FormatResult<()> {
        AstNode::<Function>::fmt_with_options(self, options, f)
    }
}

/// Check if the source text has properly closed parentheses starting with '('.
/// Returns true if the text starts with '(' and all parentheses are balanced.
fn has_closed_parentheses(source: &str) -> bool {
    let bytes = source.as_bytes().trim_ascii_start();

    // Early return if source doesn't start with '('
    if !bytes.first().is_some_and(|&b| b == b'(') {
        return false;
    }

    let mut paren_count = 0i32;
    let mut i = 0;

    while i < bytes.len() {
        match bytes[i] {
            b'(' => paren_count += 1,
            b')' => paren_count -= 1,
            b'/' if i + 1 < bytes.len() => {
                match bytes[i + 1] {
                    b'/' => {
                        // Skip to end of line comment
                        i += 2;
                        while i < bytes.len() && bytes[i] != b'\n' {
                            i += 1;
                        }
                        continue;
                    }
                    b'*' => {
                        // Skip to end of block comment
                        i += 2;
                        while i + 1 < bytes.len() {
                            if bytes[i] == b'*' && bytes[i + 1] == b'/' {
                                i += 2;
                                break;
                            }
                            i += 1;
                        }
                        continue;
                    }
                    _ => {}
                }
            }
            quote @ (b'"' | b'\'' | b'`') => {
                // Skip string literal (double-quoted, single-quoted, or template)
                i += 1;
                while i < bytes.len() {
                    match bytes[i] {
                        b if b == quote => break,
                        b'\\' if i + 1 < bytes.len() => i += 1, // Skip escaped character
                        _ => {}
                    }
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    // Return true only if parentheses are properly balanced
    paren_count == 0
}

pub fn format_type_cast_comment_node<'a, T>(
    node: impl Format<'a, T> + GetSpan,
    is_object_or_array_expression: bool,
    f: &mut Formatter<'_, 'a>,
) -> FormatResult<bool> {
    let comments = f.context().comments();
    let span = node.span();

    if !f.source_text()[span.end as usize..]
        .as_bytes()
        .trim_ascii_start()
        .first()
        .is_some_and(|&b| b == b')')
    {
        return Ok(false);
    }

    if let Some(type_cast_comment_index) = comments.get_type_cast_comment_index(span) {
        let unprinted_comments = f.context().comments().unprinted_comments();
        let type_cast_comment = &unprinted_comments[type_cast_comment_index];

        // Get the source text from the end of type cast comment to the node span
        let node_source_text =
            &f.source_text()[type_cast_comment.span.end as usize..span.end as usize];

        // `(/** @type {Number} */ (bar).zoo)`
        //                         ^^^^
        // Should wrap for `baz` rather than `baz.zoo`
        if has_closed_parentheses(node_source_text) {
            return Ok(false);
        }

        let comments = &unprinted_comments[..=type_cast_comment_index];
        write!(f, [FormatLeadingComments::Comments(comments)])?;

        f.context_mut().comments_mut().mark_last_printed_comment_as_typecast_comment();
    } else {
        if !(comments.last_typecast_comment_printed_count == comments.printed_count
            && comments.printed_count != 0)
            && comments.printed_comments().last().is_some_and(|c| {
                f.comments().is_type_cast_comment(c)
                    && f.source_text()[c.span.end as usize..span.start as usize]
                        .as_bytes()
                        .trim_ascii_start()
                        .first()
                        .is_some_and(|&b| b == b'(')
            })
        {
            f.context_mut().comments_mut().mark_last_printed_comment_as_typecast_comment();
        } else {
            // No typecast comment
            return Ok(false);
        }
    }

    if is_object_or_array_expression && !f.comments().has_comments_before(span.start) {
        write!(f, group(&format_args!("(", &format_once(|f| node.fmt(f)), ")")))?;
    } else {
        write!(
            f,
            group(&format_args!("(", soft_block_indent(&format_once(|f| node.fmt(f))), ")"))
        )?;
    }

    Ok(true)
}
