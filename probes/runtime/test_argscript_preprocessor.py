"""Native macro/if dispatch and explicit virtual include source contracts."""
import unittest

from argscript_preprocessor import Preprocessor, substitute_parameters
from argscript_variables import ArgScriptError, variable


class ParameterContracts(unittest.TestCase):
    def test_brackets_and_raw_splicing_are_different(self):
        result = substitute_parameters(('x',), ('a b',), 'emit &x &{x}_tail')
        self.assertEqual(result['text'], 'emit (a b) a b_tail')
        self.assertEqual(result['pieces'][1]['parameter'], 'x')

    def test_single_pass_case_sensitive_and_duplicate_first_wins(self):
        self.assertEqual(substitute_parameters(('x', 'x'), ('&other', 'last'), '&x')['text'], '(&other)')
        with self.assertRaises(ArgScriptError) as caught:
            substitute_parameters(('X',), ('a',), '&x')
        self.assertEqual(caught.exception.code, 'ASM008')

    def test_errors_have_separate_categories(self):
        for text, code in [('&', 'ASM005'), ('&1', 'ASM006'), ('&:x', 'ASM006'),
                            ('&{x', 'ASM007'), ('&unknown', 'ASM008')]:
            with self.subTest(text=text), self.assertRaises(ArgScriptError) as caught:
                substitute_parameters(('x',), ('v',), text)
            self.assertEqual(caught.exception.code, code)


class PreprocessorCase(unittest.TestCase):
    def host(self, sources=None):
        host, emitted = Preprocessor(sources=sources), []
        host.parser.register('emit', lambda p, i: emitted.append(
            [t['value'] for t in i['arguments']['main']]))
        return host, emitted


class MacroContracts(PreprocessorCase):
    def test_definition_delays_variables_and_malformed_arguments(self):
        host, emitted = self.host()
        host.parser.parse_stream('define M(x)\nemit $later &x\nenddef\nset later ready\ncreate M(value)')
        self.assertEqual(emitted, [['ready', 'value']])
        self.assertEqual(host.macros['m'].body[0]['text'], 'emit $later &x')

    def test_malformed_body_is_not_lexed_until_invocation(self):
        host, _ = self.host()
        host.parser.parse_stream('define M()\nemit (\nenddef')
        with self.assertRaises(ArgScriptError) as caught:
            host.parser.parse_stream('create M()', 'caller.er2')
        self.assertEqual(caught.exception.code, 'AS004')
        self.assertEqual(caught.exception.macro_frames[0]['body']['line'], 2)

    def test_define_commit_only_on_end_and_redefine_replaces(self):
        host, emitted = self.host()
        host.parser.parse_stream('define M()\nemit old\nenddef')
        with self.assertRaises(ArgScriptError) as caught:
            host.parser.parse_stream('define m()')
        self.assertEqual(caught.exception.code, 'ASM009')
        host.parser.parse_stream('redefine m()\nemit new\nenddef\ncreate M()')
        self.assertEqual(emitted, [['new']])
        other, _ = self.host()
        with self.assertRaises(ArgScriptError):
            other.parser.parse_stream('define Pending()\nemit pending')
        self.assertNotIn('pending', other.macros)
        other.parser.finish(True)
        self.assertNotIn('pending', other.macros)

    def test_macro_names_case_insensitive_parameters_are_not(self):
        host, emitted = self.host()
        host.parser.parse_stream('define MiXeD(X)\nemit &X\nenddef\ncreate mixed(v)')
        self.assertEqual(emitted, [['v']])

    def test_invalid_namespace_does_not_create_unwind_underflow(self):
        host, _ = self.host()
        with self.assertRaises(ArgScriptError) as caught:
            host.parser.parse_stream('namespace')
        self.assertEqual(str(caught.exception), 'Expecting at least 1 arguments')
        host.parser.finish(True)
        self.assertEqual(host.parser.blocks, [])

    def test_definition_table_is_not_scoped_by_namespace(self):
        host, emitted = self.host()
        host.parser.parse_stream('namespace N\ndefine M()\nemit globalMacro\nenddef\nend\ncreate m()')
        self.assertEqual(emitted, [['globalMacro']])

    def test_wrong_count_and_unknown_macro_fail_before_body(self):
        host, emitted = self.host()
        host.parser.parse_stream('define M(x)\nemit &x\nenddef')
        for text, code in [('create M()', 'ASM003'), ('create absent()', 'ASM011'),
                           ('create absent(-)', 'AS005')]:
            with self.assertRaises(ArgScriptError) as caught:
                host.parser.parse_stream(text)
            self.assertEqual(caught.exception.code, code)
        self.assertEqual(emitted, [])

    def test_body_replay_restrips_parameter_injected_comments(self):
        host, emitted = self.host()
        host.parser.parse_stream('define M(x)\nemit &{x} tail\nenddef\ncreate M("# comment")')
        self.assertEqual(emitted, [[]])

    def test_nested_macro_and_caller_scope_are_preserved(self):
        host, emitted = self.host()
        host.parser.parse_stream('define Inner(x)\nemit &x $local\nenddef\n'
                                 'define Outer(x)\ncreate Inner(&x)\nenddef\n'
                                 'namespace N\nset local here\ncreate Outer(value)\nend')
        self.assertEqual(emitted, [['value', 'here']])
        self.assertEqual(host.parser.variables.scope, '')
        self.assertEqual(variable(host.parser.variables, 'N:local'), 'here')

    def test_macro_may_open_block_closed_by_caller(self):
        host, emitted = self.host()
        host.parser.parse_stream('define Open()\nnamespace N\nenddef\ncreate Open()\n'
                                 'set local yes\nemit $local\nend')
        self.assertEqual(emitted, [['yes']])

    def test_undefine_silent_missing_and_normal_missing(self):
        host, _ = self.host()
        host.parser.parse_stream('define M()\nenddef\nundefine m\nsundefine m')
        self.assertNotIn('m', host.macros)
        with self.assertRaises(ArgScriptError) as caught:
            host.parser.parse_stream('undefine m')
        self.assertEqual(caught.exception.code, 'ASM010')

    def test_screate_is_explicitly_unrecovered_not_silently_swallowed(self):
        host, _ = self.host()
        with self.assertRaises(ArgScriptError) as caught:
            host.parser.parse_stream('screate missing()')
        self.assertEqual(caught.exception.code, 'ASM019')

    def test_recursion_has_explicit_host_limit(self):
        host, _ = self.host()
        host.parser.parse_stream('define M()\ncreate M()\nenddef')
        with self.assertRaises(ArgScriptError) as caught:
            host.parser.parse_stream('create M()')
        self.assertEqual(caught.exception.code, 'ASM099')
        self.assertEqual(host.expansion_depth, 0)

    def test_failure_keeps_definition_call_chain_and_generated_coordinates(self):
        host, _ = self.host()
        host.parser.parse_stream('define Inner()\nunknown\nenddef\n'
                                 'define Outer()\ncreate Inner()\nenddef', 'defs.er2')
        with self.assertRaises(ArgScriptError) as caught:
            host.parser.parse_stream('create Outer()', 'caller.er2')
        error = caught.exception
        self.assertEqual(error.code, 'ASP001')
        self.assertIn('<macro Inner@defs.er2:2>', error.file)
        self.assertEqual([f['macro'] for f in error.macro_frames], ['Inner', 'Outer'])
        self.assertEqual(error.macro_frames[-1]['call']['file'], 'caller.er2')
        self.assertEqual(error.macro_frames[0]['body']['file'], 'defs.er2')


class ConditionContracts(PreprocessorCase):
    def test_inactive_text_is_consumed_before_substitution_and_lexing(self):
        host, emitted = self.host()
        host.parser.parse_stream('if false\nunknown $missing (\nelse\nemit active\nendif')
        self.assertEqual(emitted, [['active']])

    def test_elseif_evaluated_only_when_no_clause_has_triggered(self):
        host, emitted = self.host()
        host.parser.parse_stream('if false\nelseif (2>1)\nemit yes\n'
                                 'elseif $missing\nemit never\nelse\nemit never\nendif')
        self.assertEqual(emitted, [['yes']])

    def test_nested_if_is_evaluated_even_under_inactive_parent(self):
        host, _ = self.host()
        with self.assertRaises(ArgScriptError) as caught:
            host.parser.parse_stream('if false\nif $missing\nendif\nendif')
        self.assertEqual(caught.exception.code, 'ASV005')

    def test_nested_if_parent_gates_all_clauses(self):
        host, emitted = self.host()
        host.parser.parse_stream('if false\nif true\nemit never\nelse\nemit never\nendif\n'
                                 'else\nemit outer\nendif')
        self.assertEqual(emitted, [['outer']])
        self.assertEqual(host.conditions.frames, [])

    def test_repeated_else_is_consumed_without_reactivating(self):
        host, emitted = self.host()
        host.parser.parse_stream('if false\nelse\nemit first\nelse\nemit never\nendif')
        self.assertEqual(emitted, [['first']])

    def test_if_prefix_is_not_a_token_boundary_check(self):
        host, _ = self.host()
        with self.assertRaises(ArgScriptError) as caught:
            host.parser.parse_stream('if false\niffy\nendif')
        self.assertEqual(caught.exception.code, 'ASP001')

    def test_uppercase_nested_if_is_consumed_in_inactive_branch(self):
        host, emitted = self.host()
        host.parser.parse_stream('if false\nIF $missing\nelse\nemit active\nENDIF')
        self.assertEqual(emitted, [['active']])

    def test_elseif_prefix_is_consumed_without_command_name_validation(self):
        host, emitted = self.host()
        host.parser.parse_stream('if false\nelseifAnything true\nemit active\nendif')
        self.assertEqual(emitted, [['active']])

    def test_unbalanced_if_and_cleanup(self):
        host, _ = self.host()
        with self.assertRaises(ArgScriptError) as caught:
            host.parser.parse_stream('if true')
        self.assertEqual(caught.exception.code, 'ASP003')
        host.parser.finish(True)
        self.assertEqual(host.conditions.frames, [])

    def test_condition_expression_error_has_source_argument_reference(self):
        host, _ = self.host()
        source = 'emit first\r\nif #< removed #> (1/0)'
        with self.assertRaises(ArgScriptError) as caught:
            host.parser.parse_stream(source, 'condition.er2')
        error = caught.exception
        self.assertEqual((error.file, error.line), ('condition.er2', 2))
        self.assertEqual(source[error.start:error.end], '(1/0)')

    def test_elseif_error_attributes_whole_source_line(self):
        host, _ = self.host()
        source = 'if false\r\n  elseif $missing\r\nendif'
        with self.assertRaises(ArgScriptError) as caught:
            host.parser.parse_stream(source)
        error = caught.exception
        self.assertEqual(source[error.start:error.end], '  elseif $missing')


class IncludeContracts(PreprocessorCase):
    def test_relative_include_shared_variables_and_restore_directory(self):
        host, emitted = self.host({'root/main.er2': 'set x yes\ninclude sub/part.er2\nemit $y',
                                  'root/sub/part.er2': 'emit $x\nset y ready'})
        host.parse_source('root/main.er2')
        self.assertEqual(emitted, [['yes'], ['ready']])
        self.assertEqual((host.base, host.directories, host.active_files), ('', [], []))
        self.assertEqual(len(host.files), 2)

    def test_missing_sinclude_is_silent_but_parse_failure_is_not(self):
        host, emitted = self.host({'root/main': 'sinclude missing\nemit yes'})
        host.parse_source('root/main')
        self.assertEqual(emitted, [['yes']])
        host, _ = self.host({'root/main': 'sinclude bad', 'root/bad': 'unknown'})
        with self.assertRaises(ArgScriptError) as caught:
            host.parse_source('root/main')
        self.assertEqual((caught.exception.file, caught.exception.line), ('root/bad', 1))
        self.assertEqual(caught.exception.include_stack, ['root/main', 'root/bad'])

    def test_drive_path_and_pushdir_replace_without_normalization(self):
        host, emitted = self.host({'root/main': 'pushDir custom/\ninclude part\npopDir\ninclude D:/abs',
                                  'custom/part': 'emit custom', 'D:/abs': 'emit drive'})
        host.parse_source('root/main')
        self.assertEqual(emitted, [['custom'], ['drive']])
        self.assertEqual(host.base, '')

    def test_macro_include_uses_callers_directory_not_definition_directory(self):
        host, emitted = self.host({'defs/macros': 'define M()\ninclude child\nenddef',
                                  'city/main': 'create M()', 'city/child': 'emit caller'})
        host.parse_source('defs/macros')
        host.parse_source('city/main')
        self.assertEqual(emitted, [['caller']])

    def test_child_stream_may_not_leave_block_depth_changed(self):
        host, _ = self.host({'main': 'include child', './child': 'namespace N'})
        with self.assertRaises(ArgScriptError) as caught:
            host.parse_source('main')
        self.assertEqual((caught.exception.code, caught.exception.file), ('ASP002', './child'))
        self.assertEqual(host.base, '')
        host.parser.finish(True)

    def test_cycle_is_a_host_guard_and_directories_unwind(self):
        host, _ = self.host({'root/main': 'include main'})
        with self.assertRaises(ArgScriptError) as caught:
            host.parse_source('root/main')
        self.assertEqual(caught.exception.code, 'ASM017')
        self.assertEqual((host.base, host.directories, host.active_files), ('', [], []))

    def test_active_missing_include_and_root_path_gap_are_visible(self):
        for source, code in [('include missing', 'ASM016'), ('include /rooted', 'ASM015')]:
            host, _ = self.host({'root/main': source})
            with self.assertRaises(ArgScriptError) as caught:
                host.parse_source('root/main')
            self.assertEqual(caught.exception.code, code)

    def test_include_may_define_macro_for_parent_and_nested_error_stays_deepest(self):
        host, emitted = self.host({'root/main': 'include lib\ncreate M()',
                                  'root/lib': 'define M()\nemit fromLib\nenddef'})
        host.parse_source('root/main')
        self.assertEqual(emitted, [['fromLib']])
        host, _ = self.host({'root/main': 'include mid', 'root/mid': 'include leaf',
                             'root/leaf': 'emit before\nunknown'})
        with self.assertRaises(ArgScriptError) as caught:
            host.parse_source('root/main')
        error = caught.exception
        self.assertEqual((error.file, error.line), ('root/leaf', 2))
        self.assertEqual(error.include_stack, ['root/main', 'root/mid', 'root/leaf'])
        self.assertEqual([row['failure']['file'] for row in host.parser.trace if 'failure' in row],
                         ['root/leaf'] * 3)

    def test_block_comment_depth_is_shared_across_include_streams(self):
        host, emitted = self.host({'root/main': 'include child\nignored\n#>emit after',
                                  'root/child': '#<'})
        host.parse_source('root/main')
        self.assertEqual(emitted, [['after']])


if __name__ == '__main__':
    unittest.main()
