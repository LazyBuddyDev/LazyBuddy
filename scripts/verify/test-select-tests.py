"""Unit selection must not silently expand a feature change into a full suite."""
import importlib.util
from pathlib import Path
import unittest
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
TMP = ROOT / 'tmp'
TMP.mkdir(exist_ok=True)
spec = importlib.util.spec_from_file_location('selection', ROOT/'scripts/check-changes.py')
selection = importlib.util.module_from_spec(spec)
spec.loader.exec_module(selection)


class Selection(unittest.TestCase):
    def test_build_entry_changes_run_windows_powershell_regression_only(self):
        for path in ('start.bat', 'scripts/build-and-run.ps1', 'scripts/verify/test-build-entry.ps1'):
            self.assertEqual(selection.plan([path]), [['powershell.exe', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', 'scripts/verify/test-build-entry.ps1']])

    def repository(self):
        temporary = tempfile.TemporaryDirectory(prefix='qa-selection-', dir=TMP)
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        def git(*args):
            return subprocess.check_output(['git', *args], cwd=root, text=True, stderr=subprocess.DEVNULL).strip()
        git('init', '--initial-branch=main')
        def commit(name):
            (root / name).write_text(name, encoding='utf-8')
            git('add', name)
            git('-c', 'user.name=Unit', '-c', 'user.email=unit@example.invalid', 'commit', '-m', name)
            return git('rev-parse', 'HEAD')
        return root, git, commit

    def test_missing_base_is_fetched_before_comparing(self):
        root, git, commit = self.repository()
        base = commit('baseline.rs')
        commit('one.ts')
        commit('two.rs')
        clone = root / 'shallow'
        git('clone', '--depth=1', root.as_uri(), str(clone))
        self.assertNotEqual(subprocess.run(['git', 'cat-file', '-e', base], cwd=clone,
                                          capture_output=True).returncode, 0)
        self.assertEqual(sorted(selection.changed_paths(base, clone)), ['one.ts', 'two.rs'])

    def test_unavailable_rewritten_base_covers_the_whole_push(self):
        root, git, commit = self.repository()
        commit('baseline.rs')
        first = commit('one.ts')
        commit('two.rs')
        self.assertEqual(sorted(selection.changed_paths('f' * 40, root, first)), ['one.ts', 'two.rs'])

    def test_unavailable_base_does_not_silently_check_only_the_last_commit(self):
        root, git, commit = self.repository()
        commit('baseline.rs')
        commit('one.ts')
        with self.assertRaisesRegex(RuntimeError, 'Comparison base is unavailable'):
            selection.changed_paths('f' * 40, root)

    def test_rewritten_push_starting_at_root_compares_the_complete_tree(self):
        root, git, commit = self.repository()
        first = commit('one.ts')
        commit('two.rs')
        self.assertEqual(sorted(selection.changed_paths('f' * 40, root, first)), ['one.ts', 'two.rs'])

    def test_new_branch_zero_before_compares_main_instead_of_all_files(self):
        with tempfile.TemporaryDirectory(prefix='qa-selection-',dir=TMP) as temporary:
            root = Path(temporary)
            def git(*args):
                return subprocess.check_output(['git', *args], cwd=root, text=True, stderr=subprocess.DEVNULL)
            git('init', '--initial-branch=main')
            (root/'existing.rs').write_text('baseline', encoding='utf-8')
            git('add', '.')
            git('-c', 'user.name=Unit', '-c', 'user.email=unit@example.invalid', 'commit', '-m', 'baseline')
            git('update-ref', 'refs/remotes/origin/main', 'HEAD')
            git('switch', '-c', 'feature')
            (root/'prototype.ts').write_text('changed', encoding='utf-8')
            git('add', '.')
            git('-c', 'user.name=Unit', '-c', 'user.email=unit@example.invalid', 'commit', '-m', 'feature')
            self.assertEqual(selection.changed_paths('0'*40, root), ['prototype.ts'])

    def test_updater_changes_select_own_unit_tests_only(self):
        commands = selection.plan(['apps/desktop/src/main.rs', 'apps/desktop/src-tauri/src/host.rs', 'crates/app-updater/src/tests.rs'])
        tests = [command for command in commands if command[:2] == ['cargo', 'test']]
        self.assertIn(['cargo', 'test', '--locked', '-p', 'lazybuddy', 'host::tests'], tests)
        self.assertIn(['cargo', 'test', '--locked', '-p', 'lazybuddy-app-updater', 'tests'], tests)
        self.assertTrue(all('--workspace' not in command for command in commands))
        self.assertTrue(all('test-shell-ui.py' not in str(command) for command in commands))

    def test_dependency_changes_compile_without_forcing_all_tests(self):
        commands = selection.plan(['Cargo.lock'])
        self.assertEqual(len(commands), 1)
        self.assertEqual(commands[0][:2], ['cargo', 'check'])

    def test_documents_do_not_trigger_tests(self):
        self.assertEqual(selection.plan(['README.md', 'TASKS.md']), [])


if __name__ == '__main__':
    unittest.main()
