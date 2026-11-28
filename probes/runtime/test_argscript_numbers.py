"""Source-derived integer reader, recursive-descent, and command-path cases."""
import unittest

from argscript_argument_spec import ArgInfo, ArgumentSpec, VECTOR, checked_command, parse_argv
from argscript_expression import integer_expression_converters, parse_expression
from argscript_numbers import U64_MAX, int32, read_int32, read_integer
from argscript_parser import Parser
from argscript_variables import ArgScriptError, set_variable_raw


class RawIntegerContracts(unittest.TestCase):
    def test_decimal_prefix_and_end_cursor(self):
        for text, value, end in [('  -12tail', -12, 5), ('+7.5', 7, 2),
                                 ('09', 9, 2), ('0x10', 0, 1)]:
            with self.subTest(text=text):
                parsed = read_integer(text)
                self.assertEqual((int32(parsed.bits), parsed.end), (value, end))

    def test_no_digit_restores_original_pointer(self):
        for text in ('', '   ', ' -x', '+', 'word'):
            result = read_integer(text)
            self.assertEqual((result.bits, result.end, result.digit_read), (0, 0, False))

    def test_base_zero_and_hex_prefix(self):
        self.assertEqual(read_integer('010', base=0).bits, 8)
        self.assertEqual(read_integer('09', base=0).end, 1)
        self.assertEqual(read_integer('0Xff!', base=0).bits, 255)
        self.assertEqual(read_integer('0x', base=0).end, 0)
        self.assertEqual(read_integer('z', base=36).bits, 35)
        self.assertEqual(read_integer('123', base=1).end, 0)

    def test_signed_64_clamp_then_low_word(self):
        self.assertEqual(read_int32('2147483648'), -2147483648)
        self.assertEqual(read_int32('4294967296'), 0)
        result = read_integer('9223372036854775808')
        self.assertTrue(result.overflow)
        self.assertEqual(result.bits, (1 << 63) - 1)
        self.assertEqual(read_int32('9223372036854775808'), -1)
        self.assertEqual(read_int32('-9223372036854775809'), 0)

    def test_unsigned_overflow_and_negative_wrap(self):
        result = read_integer('18446744073709551616tail', unsigned=True)
        self.assertEqual((result.bits, result.end, result.overflow), (U64_MAX, 20, True))
        self.assertEqual(read_integer('-1', unsigned=True).bits, U64_MAX)
        self.assertEqual(read_integer('-18446744073709551616', unsigned=True).bits, 1)

    def test_cursor_and_unsupported_encoding_guards(self):
        self.assertEqual(read_integer('xx 12suffix', start=2).end, 5)
        for text in ('１２', '1\0ignored'):
            with self.assertRaises(ArgScriptError):
                read_integer(text)


class IntegerExpressionContracts(unittest.TestCase):
    def test_precedence_and_parentheses(self):
        for text, value in [('2+3*4', 14), ('(2+3)*4', 20), ('8/2*3', 12),
                            ('2^3^2', 512), ('-2^2', 4), ('2^-3', 1),
                            ('0^0', 1), ('+++--3', 3)]:
            with self.subTest(text=text):
                self.assertEqual(parse_expression(text), value)

    def test_division_and_modulo_truncate_towards_zero(self):
        self.assertEqual(parse_expression('-7/3'), -2)
        self.assertEqual(parse_expression('-7%3'), -1)
        self.assertEqual(parse_expression('7%-3'), 1)

    def test_decimal_leading_zero_preprocessing_and_hex(self):
        self.assertEqual(parse_expression('010+0009'), 19)
        self.assertEqual(parse_expression('0x10+0Xf'), 31)
        self.assertEqual(parse_expression('000x10'), 16)

    def test_literal_range_check_precedes_unary_minus(self):
        for text in ('2147483648', '-2147483648', '0xffffffff'):
            with self.subTest(text=text), self.assertRaises(ArgScriptError) as caught:
                parse_expression(text)
            self.assertEqual(str(caught.exception), 'Out of range')
        self.assertEqual(parse_expression('-2147483647-1'), -2147483648)

    def test_arithmetic_wrap_and_unsigned_literal_mode(self):
        self.assertEqual(parse_expression('2147483647+1'), -2147483648)
        self.assertEqual(parse_expression('65536*65536'), 0)
        self.assertEqual(parse_expression('2^32'), 0)
        self.assertEqual(parse_expression('0xffffffff', 'uint'), 0xffffffff)
        self.assertEqual(parse_expression('-1', 'uint'), 0xffffffff)

    def test_prefix_matching_supported_integer_builtins(self):
        self.assertEqual(parse_expression('abs(-3)+sqr(4)'), 19)
        self.assertEqual(parse_expression('a(-3)+s(4)'), 19)

    def test_empty_bad_and_trailing_text_have_distinct_messages(self):
        for text, message in [('', 'empty expression'), ('   ', 'Bad integer expression'),
                               ('1tail', 'Garbage at end of integer expression'),
                               ('(1+2', "Expected ')'"), ('1+', 'Bad integer expression')]:
            with self.subTest(text=text), self.assertRaises(ArgScriptError) as caught:
                parse_expression(text)
            self.assertEqual(str(caught.exception), message)
        self.assertEqual(parse_expression('  1 + 2 \t'), 3)

    def test_faults_and_unrecovered_branches_are_host_errors(self):
        for text, code in [('1/0', 'ASX098'), ('1%0', 'ASX098'),
                            ('(-2147483647-1)/-1', 'ASX098'),
                            ('true', 'ASX005'), ('floor(1.2)', 'ASX005')]:
            with self.subTest(text=text), self.assertRaises(ArgScriptError) as caught:
                parse_expression(text)
            self.assertEqual(caught.exception.code, code)

    def test_complexity_is_bounded_without_native_faults(self):
        with self.assertRaises(ArgScriptError) as caught:
            parse_expression('(' * 100 + '1' + ')' * 100)
        self.assertEqual(caught.exception.code, 'ASX099')


class BooleanExpressionContracts(unittest.TestCase):
    def test_constants_prefixes_and_case_sensitive_words(self):
        for text in ('true', 'on', 't', 'tru', 'o'):
            self.assertTrue(parse_expression(text, 'bool'))
        for text in ('false', 'off', 'f', 'of'):
            self.assertFalse(parse_expression(text, 'bool'))
        with self.assertRaises(ArgScriptError):
            parse_expression('TRUE', 'bool')

    def test_logical_precedence_negation_and_parentheses(self):
        self.assertTrue(parse_expression('true or false and false', 'bool'))
        self.assertFalse(parse_expression('(true or false) and false', 'bool'))
        self.assertTrue(parse_expression('not false and !0', 'bool'))
        self.assertTrue(parse_expression('not0', 'bool'))

    def test_both_operands_are_evaluated(self):
        for text in ('true or 1/0', 'false and 1/0'):
            with self.subTest(text=text), self.assertRaises(ArgScriptError) as caught:
                parse_expression(text, 'bool')
            self.assertEqual(caught.exception.code, 'ASX098')

    def test_relations_are_right_associative(self):
        self.assertTrue(parse_expression('3>2>1', 'bool'))
        self.assertFalse(parse_expression('1==2==0', 'bool'))
        self.assertTrue(parse_expression('2+3*4>=14 and 7!=2', 'bool'))
        self.assertTrue(parse_expression('2<=2', 'bool'))

    def test_numeric_parentheses_use_boolean_mode(self):
        self.assertTrue(parse_expression('(1<2)+1==2', 'bool'))
        with self.assertRaises(ArgScriptError) as caught:
            parse_expression('1=1', 'bool')
        self.assertEqual(caught.exception.code, 'ASX006')

    def test_logical_words_are_not_python_operators(self):
        for text in ('true && false', 'true || false', '1 << 2'):
            with self.subTest(text=text), self.assertRaises(ArgScriptError):
                parse_expression(text, 'bool')


class NumericPipelineContracts(unittest.TestCase):
    def test_converters_resolve_numeric_requests_in_spec(self):
        spec = ArgumentSpec(arguments=(ArgInfo('x', 2, destination='x'),
                                       ArgInfo('b', 1, destination='b')))
        result = parse_argv(spec, ['cmd', '2+3*4', 'true and !0'],
                            converters=integer_expression_converters())
        self.assertTrue(result.fully_converted)
        self.assertEqual([w['value'] for w in result.writes], [14, True])

    def test_raw_int_bool_and_vector_routes(self):
        spec = ArgumentSpec(arguments=(ArgInfo('x', 2 | VECTOR, destination='x'),
                                       ArgInfo('b', 1, destination='b')))
        result = parse_argv(spec, ['cmd', '2tail -3.5', 'true'], parser_present=False,
                            converters=integer_expression_converters())
        self.assertEqual([w['value'] for w in result.writes], [2, -3, False])
        self.assertTrue(result.fully_converted)

    def test_float_request_stays_unresolved(self):
        spec = ArgumentSpec(arguments=(ArgInfo('x', 3, destination='x'),))
        result = parse_argv(spec, ['cmd', '1.5'], converters=integer_expression_converters())
        self.assertFalse(result.fully_converted)
        self.assertEqual(result.unresolved[0].route, 'ParseFloat')

    def test_text_substitution_to_numeric_consumer(self):
        parser, seen = Parser(), []
        parser.variables = set_variable_raw(parser.variables, 'n', '2+3')
        spec = ArgumentSpec(arguments=(ArgInfo('x', 2, destination='x'),))
        parser.register('number', checked_command(spec, lambda p, i, r: seen.extend(r.writes),
                                                 converters=integer_expression_converters()))
        parser.parse_stream('number ($n*4)')
        self.assertEqual(seen[0]['value'], 20)

    def test_expression_failure_keeps_partial_writes_and_original_source(self):
        parser, seen = Parser(), []
        spec = ArgumentSpec(arguments=(ArgInfo('x', 2, destination='x', flag=2),
                                       ArgInfo('y', 2, destination='y', flag=3)))
        parser.register('number', checked_command(spec, lambda p, i, r: seen.append(r),
                                                 converters=integer_expression_converters()))
        source = 'number 7 #< removed #> (1/0)\r\nnumber 3 4'
        with self.assertRaises(ArgScriptError) as caught:
            parser.parse_stream(source, 'numeric.er2')
        error = caught.exception
        self.assertEqual((error.code, error.file, error.line), ('ASX098', 'numeric.er2', 1))
        self.assertEqual(source[error.start:error.end], '(1/0)')
        self.assertEqual(error.conversion_route, 'ParseInt')
        failure = parser.trace[-1]['invocation']['argumentSpecFailure']
        self.assertEqual(failure['partial']['flags'], 12)
        self.assertEqual(failure['partial']['writes'][0]['value'], 7)
        self.assertEqual(failure['argumentIndex'], 2)
        self.assertEqual(seen, [])
        self.assertEqual(len(parser.trace), 1)


if __name__ == '__main__':
    unittest.main()
