import contextlib
import io
import json
import os
import tempfile
import threading
import unittest
import urllib.error
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from unittest.mock import patch
from pathlib import Path

import notify_release
import release_manifest
from check_release_version import release_tag


class NotificationTest(unittest.TestCase):
    def test_published_version_reaches_the_authenticated_endpoint(self):
        received = []

        class Handler(BaseHTTPRequestHandler):
            def do_POST(self):
                value = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
                received.append((self.path, self.headers['Authorization'], value))
                body = json.dumps(value).encode()
                self.send_response(200)
                self.send_header('Content-Length', str(len(body)))
                self.end_headers()
                self.wfile.write(body)

            def log_message(self, *args):
                pass

        http = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        thread = threading.Thread(target=http.serve_forever, daemon=True)
        thread.start()
        try:
            with patch.dict(os.environ, {
                'VITE_FASTCLOUD_SERVER_URL': f'http://127.0.0.1:{http.server_port}',
                'FASTCLOUD_RELEASE_NOTIFY_TOKEN': 'test-notification-secret',
            }), contextlib.redirect_stdout(io.StringIO()):
                notify_release.main()
            self.assertEqual(received[0][0], '/v1/updates/published')
            self.assertEqual(received[0][1], 'Bearer test-notification-secret')
            self.assertRegex(received[0][2]['version'], r'^\d+\.\d+\.\d+(?:-[a-z])?$')
        finally:
            http.shutdown()
            http.server_close()
            thread.join()

    def test_missing_secret_skips_push_without_failing_the_existing_release(self):
        output = io.StringIO()
        with patch.dict(os.environ, {'FASTCLOUD_RELEASE_NOTIFY_TOKEN': ''}), \
                patch.object(notify_release.urllib.request, 'urlopen') as request, \
                contextlib.redirect_stdout(output):
            notify_release.main()
        request.assert_not_called()
        self.assertIn('::warning::', output.getvalue())

    def test_network_failure_retries_without_exposing_the_secret_or_origin(self):
        with patch.dict(os.environ, {
            'VITE_FASTCLOUD_SERVER_URL': 'https://private-server.example',
            'FASTCLOUD_RELEASE_NOTIFY_TOKEN': 'test-notification-secret',
        }), patch.object(notify_release.urllib.request, 'urlopen',
                         side_effect=urllib.error.URLError('private-server.example test-notification-secret')) as request, \
                patch.object(notify_release.time, 'sleep'):
            with self.assertRaises(SystemExit) as failure:
                notify_release.main()
        self.assertEqual(request.call_count, 3)
        self.assertNotIn('private-server.example', str(failure.exception))
        self.assertNotIn('test-notification-secret', str(failure.exception))


class ReleaseNamingTest(unittest.TestCase):
    def test_lettered_tag_points_at_the_actual_github_release(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            config = root / 'config.json'
            config.write_text('{"version":"0.2.1-a"}', encoding='utf-8')
            installers = root / 'installers'
            installers.mkdir()
            installer = installers / 'Fastcloud_0.2.1-a_x64-setup.exe'
            installer.write_bytes(b'test installer')
            installer.with_suffix('.exe.sig').write_text('test signature', encoding='utf-8')
            dist = root / 'dist'
            with patch.object(release_manifest, 'CONFIG', config), \
                    patch.object(release_manifest, 'INSTALLERS', installers), \
                    patch.object(release_manifest, 'DIST', dist), \
                    patch.dict(os.environ, {'GITHUB_REF_NAME': 'v0.2.1a'}), \
                    contextlib.redirect_stdout(io.StringIO()):
                release_manifest.main()
            manifest = json.loads((dist / 'latest.json').read_text(encoding='utf-8'))
            self.assertEqual(manifest['version'], '0.2.1-a')
            self.assertTrue(manifest['platforms']['windows-x86_64']['url'].endswith(
                '/v0.2.1a/Fastcloud_0.2.1-a_x64-setup.exe'))

    def test_a_mismatched_tag_is_rejected_before_publication(self):
        with patch.dict(os.environ, {'GITHUB_REF_NAME': 'v0.2.1'}):
            with self.assertRaises(SystemExit):
                release_tag('0.2.0')


if __name__ == '__main__':
    unittest.main()
