"""Typed setters, registry semantics and native EvalString/wildcard contracts."""
import unittest

from argscript_expression import parse_expression
from argscript_functions import (ExprFunction, FunctionEnvironment, RegisteredExpression,
                                 eval_string, install_meta_extensions, string_call, wildcard_match)
from argscript_preprocessor import Preprocessor
from argscript_real import numeric_converters, parse_real
from argscript_variables import ArgScriptError, variable


class FunctionContracts(unittest.TestCase):
    def host(self):
        host = Preprocessor()
        environment = install_meta_extensions(host, versions=(2, 13))
        return host, environment

    def test_registry_exact_case_replace_and_remove(self):
        environment = FunctionEnvironment()
        def const(value):
            return ExprFunction(lambda r: string_call(r, 0) or value)
        environment.register('Custom', const(7))
        self.assertEqual(environment.parse('Custom()+1'), 8)
        with self.assertRaises(ArgScriptError):
            environment.parse('custom()')
        environment.register('Custom', const(9))
        self.assertEqual(environment.parse('Custom()'), 9)
        environment.register('Custom', None)
        with self.assertRaises(ArgScriptError):
            environment.parse('Custom()')

    def test_bool_override_and_default_int_to_bool(self):
        environment = FunctionEnvironment()
        environment.register('custom', ExprFunction(lambda r: string_call(r, 0) or 7,
                                                    boolean=lambda r: string_call(r, 0) or 0))
        self.assertEqual(environment.parse('custom()'), 7)
        self.assertFalse(environment.parse('custom()', 'bool'))
        environment.register('custom', ExprFunction(lambda r: string_call(r, 0) or 7))
        self.assertTrue(environment.parse('custom()', 'bool'))

    def test_callback_cursor_must_be_fully_consumed(self):
        environment = FunctionEnvironment()
        environment.register('bad', ExprFunction(lambda r: 7))
        with self.assertRaises(ArgScriptError) as caught:
            environment.parse('bad()')
        self.assertIn('Garbage at end', str(caught.exception))

    def test_registered_real_result_is_stored_as_float(self):
        environment = FunctionEnvironment()
        environment.register('custom', ExprFunction(lambda r: 0,
                                  real=lambda r: string_call(r, 0) or 16777217.0))
        self.assertEqual(environment.parse('custom()', 'real'), 16777216.0)

    def test_native_builtins_take_precedence_over_registry(self):
        environment = FunctionEnvironment()
        environment.register('abs', ExprFunction(lambda r: 999))
        self.assertEqual(environment.parse('abs(-7)'), 7)

    def test_eval_string_parentheses_quotes_and_tab_delimiter_difference(self):
        reader = RegisteredExpression(' (("a,b c")) , next', FunctionEnvironment())
        self.assertEqual(eval_string(reader), 'a,b c')
        reader.expect(',')
        self.assertEqual(eval_string(reader), 'next')
        reader = RegisteredExpression('a\tb,c', FunctionEnvironment())
        self.assertEqual(eval_string(reader), 'a\tb')

    def test_wildcard_has_only_star_and_question_syntax(self):
        self.assertTrue(wildcard_match('CriminalRobber', 'criminal*'))
        self.assertTrue(wildcard_match('ab', '?b'))
        self.assertTrue(wildcard_match('a[b]', 'a[b]'))
        self.assertFalse(wildcard_match('ab', 'a[b]'))
        self.assertTrue(wildcard_match('', '*'))
        self.assertFalse(wildcard_match('', '**'))
        self.assertFalse(wildcard_match('a', 'a**'))
        self.assertFalse(wildcard_match('', '?'))
        self.assertTrue(wildcard_match('abc', 'a**?c'))

    def test_match_eq_and_boolean_in_real_rejected(self):
        _, environment = self.host()
        self.assertTrue(environment.parse('match(CriminalRobber,"criminal*")', 'bool'))
        self.assertEqual(environment.parse('eq((abc), "ABC")'), 1)
        self.assertEqual(environment.parse('eq(ab,abc)'), 0)
        with self.assertRaises(ArgScriptError) as caught:
            environment.parse('match(a,a)', 'real')
        self.assertEqual(str(caught.exception), 'boolean function')

    def test_live_existence_and_global_only_command_exists(self):
        host, environment = self.host()
        self.assertFalse(environment.parse('varExists(x)', 'bool'))
        host.parser.parse_stream('set x ""\ndefine M()\nenddef')
        self.assertTrue(environment.parse('varExists(x)', 'bool'))
        self.assertTrue(environment.parse('defExists(m)', 'bool'))
        self.assertTrue(environment.parse('commandExists(SET)', 'bool'))
        host.parser.start_block({'localOnly': lambda p, i: None})
        self.assertFalse(environment.parse('commandExists(localOnly)', 'bool'))
        host.parser.end_block()

    def test_versions_are_explicit_and_default_not_guessed(self):
        _, environment = self.host()
        self.assertEqual(environment.parse('minVersion()+maxVersion()'), 15)
        other = Preprocessor()
        unknown = install_meta_extensions(other)
        with self.assertRaises(ArgScriptError):
            unknown.parse('minVersion()')


class TypedSetterContracts(unittest.TestCase):
    def host(self):
        host = Preprocessor()
        install_meta_extensions(host)
        return host

    def test_integer_unsigned_bool_store_decimal_strings(self):
        host = self.host()
        host.parser.parse_stream('seti a (2+3*4)\nsetu u -1\nsetb yes (2>1)\nsetb no false')
        self.assertEqual([variable(host.parser.variables, name) for name in ('a','u','yes','no')],
                         ['14','4294967295','1','0'])

    def test_float_fixed_six_decimals_and_double_return_boundary(self):
        host = self.host()
        host.parser.parse_stream('setf value (1/8)\nsetf large (16777216+1)')
        self.assertEqual(variable(host.parser.variables, 'value'), '0.125000')
        self.assertEqual(variable(host.parser.variables, 'large'), '16777217.000000')
        self.assertEqual(parse_real('16777216+1'), 16777216.0)

    def test_float_buffer_truncates_to_31_bytes(self):
        host = self.host()
        host.parser.parse_stream('setf huge 1e35')
        stored = variable(host.parser.variables, 'huge')
        self.assertEqual(len(stored), 31)
        self.assertNotIn('.', stored)

    def test_variables_substituted_once_and_scope_preserved(self):
        host = self.host()
        host.parser.parse_stream('set expr 2+3\nnamespace N\nseti total ($expr*4)\nend')
        self.assertEqual(variable(host.parser.variables, 'N:total'), '20')

    def test_failure_preserves_old_value_and_original_source(self):
        host = self.host()
        source = 'set value old\r\nseti value #< removed #> (1/0)\r\nset later never'
        with self.assertRaises(ArgScriptError) as caught:
            host.parser.parse_stream(source)
        self.assertEqual(source[caught.exception.start:caught.exception.end], '(1/0)')
        self.assertEqual(variable(host.parser.variables, 'value'), 'old')
        self.assertIsNone(variable(host.parser.variables, 'later'))

    def test_macro_condition_uses_registered_match(self):
        host = self.host()
        source = ('define Pick(kind)\nif (match(&{kind},"Criminal*"))\n'
                  'seti picked 1\nelse\nseti picked 0\nendif\nenddef\ncreate Pick(CriminalRobber)')
        host.parser.parse_stream(source)
        self.assertEqual(variable(host.parser.variables, 'picked'), '1')

    def test_old_default_environment_stays_unextended(self):
        host = Preprocessor()
        with self.assertRaises(ArgScriptError) as caught:
            host.parser.parse_stream('seti a 1')
        self.assertEqual(caught.exception.code, 'ASP001')
        with self.assertRaises(ArgScriptError):
            parse_expression('match(a,a)', 'bool')


if __name__ == '__main__':
    unittest.main()
