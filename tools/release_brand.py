"""Keep historical artifact URLs immutable while branding new releases."""
import re

def asset_prefix(version):
    match = re.fullmatch(r'(\d+)\.(\d+)\.(\d+)(?:-[a-z])?', version)
    if not match:
        raise ValueError('Invalid release version')
    return 'Veirune' if tuple(map(int, match.groups())) >= (0, 4, 15) else 'Fastcloud'
