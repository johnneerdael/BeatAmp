"""Inspect an immutable snapshot of available corpus exports without touching workers.

Run with uv run --with jsonschema python tools/audit-predictions.py --help.
Only the report directory is written. No producer code is imported or executed.
"""
import argparse
from pathlib import Path
from collections import Counter, defaultdict
import datetime
import hashlib
import json
import math
import zipfile
from jsonschema import Draft202012Validator

parser = argparse.ArgumentParser(description='Read-only predictor export integrity audit; never runs the producer')
parser.add_argument('--input', type=Path, required=True)
parser.add_argument('--producer', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--workspace', type=Path, default=Path(__file__).resolve().parent.parent)
args = parser.parse_args()
BASE = args.input.resolve()
PRODUCER = args.producer.resolve()
OUTPUT = args.output.resolve()
WORKSPACE = args.workspace.resolve()
# Avoid writing into either live evidence or the producer checkout.
if any(OUTPUT == p or p in OUTPUT.parents for p in [BASE, PRODUCER, WORKSPACE/'corpus']):
    parser.error('output must be separate from live evidence, producer and frozen corpus')
OUTPUT.mkdir(parents=True, exist_ok=True)


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(',', ':'), allow_nan=False).encode()).hexdigest()


def file_digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    raw_manifest = (BASE/'run-manifest.json').read_bytes()
    manifest = json.loads(raw_manifest)
    run_id = digest(manifest)
    configuration = manifest['configuration']
    schema_path = PRODUCER/'tools/milk-analyzer/export-contract/source-feature-record.schema.json'
    schema = json.loads(schema_path.read_text())
    Draft202012Validator.check_schema(schema)
    validator = Draft202012Validator(schema)
    inputs = json.loads((BASE/'inputs/manifest.json').read_text())
    prepared_audio = json.loads((BASE/'inputs/audio.json').read_text())
    inventory = {p['relative_path']: p for p in manifest['presets']}
    frozen = json.loads((WORKSPACE/'corpus/manifest.json').read_text())
    frozen_sources = {p['sha256'] for p in frozen['presets']}
    errors = []
    for name, sha in inputs['files'].items():
        if file_digest(BASE/'inputs'/name) != sha:
            errors.append('Prepared input hash mismatch: '+name)
    if file_digest(BASE/'inputs/manifest.json') != configuration['prepared_inputs_sha256']:
        errors.append('Prepared input manifest hash mismatch')
    module_results = []
    for name, sha in configuration['model_modules'].items():
        path = PRODUCER/'tools/milk-analyzer'/name
        okay = path.exists() and file_digest(path) == sha
        module_results.append({'module': name, 'sha256': sha, 'current_file_matches': okay})
        if not okay:
            errors.append('Producer module differs from run identity: '+name)
    # Freeze only the list of already completed JSON files. The directory can
    # continue to grow while this independent inspection finishes.
    snapshot_files = sorted((BASE/'results').rglob('*.json'))
    status_counts = Counter()
    unknown_counts = Counter()
    unknown_reasons = Counter()
    stage_counts = Counter()
    scalar_values = defaultdict(list)
    records = []
    keys_seen = set()
    matched_frozen = []
    for path in snapshot_files:
        before = path.stat()
        raw = path.read_bytes()
        after = path.stat()
        if (before.st_size, before.st_mtime_ns) != (after.st_size, after.st_mtime_ns):
            errors.append('Result changed during read: '+str(path))
            continue
        result = json.loads(raw, parse_constant=lambda s: (_ for _ in ()).throw(ValueError('Nonfinite JSON '+s)))
        name = result['preset']['relative_path']
        row_errors = []
        expected = inventory.get(name)
        if result['run_identity'] != run_id or expected is None or result['preset'] != expected:
            row_errors.append('Run/source identity mismatch')
        source = PRODUCER/'core/src/main/assets/presets'/name
        if not source.is_file() or file_digest(source) != result['preset']['sha256']:
            row_errors.append('Source bytes unavailable or mismatched')
        status = result['status']
        status_counts[status] += 1
        if result['preset']['sha256'] in frozen_sources:
            matched_frozen.append(name)
        row = {'file': str(path), 'file_sha256': hashlib.sha256(raw).hexdigest(),
               'preset': result['preset'], 'source_path': str(source), 'status': status,
               'errors': row_errors, 'stage': result['stage']}
        if result.get('simulation') != configuration['simulation']:
            row_errors.append('Simulation context mismatch')
        if result.get('uses_rendered_reference') is not False or result.get('appearance_accuracy_verified') is not False:
            row_errors.append('Unexpected appearance/reference claim')
        feature_record = result.get('feature_record')
        if status != 'computed':
            if feature_record is not None:
                row_errors.append('Noncomputed result contains a success record')
            row['failure'] = {k: result.get(k) for k in ['error_type', 'error', 'stage']}
        else:
            row_errors.extend('Schema: '+e.message for e in validator.iter_errors(feature_record))
            context = feature_record['context']
            unsigned_record = {k: v for k, v in feature_record.items() if k != 'record_sha256'}
            checks = {'context_hash': digest(context) == feature_record['context_sha256'],
                      'record_hash': digest(unsigned_record) == feature_record['record_sha256'],
                      'domain_hash': digest(context['domain']) == context['input_hashes']['domain_sha256'],
                      'preset_hash': context['input_hashes']['preset_sha256'] == result['preset']['sha256'],
                      'audio_hash': digest(prepared_audio) == context['input_hashes']['audio_sha256'],
                      'pcm_hash': prepared_audio['pcm_sha256'] == context['input_hashes']['pcm_sha256'],
                      'model_modules': context['provenance']['model_modules'] == configuration['model_modules'],
                      'model_hash': digest(configuration['model_modules']) == context['provenance']['model_sha256'],
                      'reader_hash': context['provenance']['reader_sha256'] == configuration['binary_sha256']['milk-native-reader'],
                      'wave_binary_hash': context['provenance']['wave_binary_sha256'] == configuration['binary_sha256']['milk-wave-inputs'],
                      'extractor_hash': context['feature_extractor_sha256'] == configuration['model_modules']['source_features.py'],
                      'basis': feature_record['feature_basis'] == 'source-field-simulation'}
            row['identity_checks'] = checks
            row_errors.extend('Identity check failed: '+k for k, v in checks.items() if not v)
            features = feature_record['features']
            keys_seen.add(tuple(sorted(features)))
            if len(features) != 47:
                row_errors.append('Producer currently promises 47 fields')
            known = 0
            unknown = 0
            for key, field in features.items():
                if field['status'] == 'unknown':
                    unknown += 1
                    unknown_counts[key] += 1
                    unknown_reasons.update(field['unknown_reasons'])
                    if field['value'] is not None or not field['unknown_reasons']:
                        row_errors.append('Invalid unknown value/reason: '+key)
                elif field['status'] == 'computed':
                    known += 1
                    if key == 'flashing.events':
                        events = field['value']
                        if len(events) != configuration['simulation']['frames'] - 1:
                            row_errors.append('Event count mismatch')
                        previous = None
                        for event in events:
                            if event['dt'] <= 0 or not math.isclose(event['dt'], event['end_time']-event['start_time'], abs_tol=1e-12):
                                row_errors.append('Invalid transition timing')
                            if previous is not None and event['start_time'] != previous:
                                row_errors.append('Noncontiguous transition timeline')
                            previous = event['end_time']
                    elif isinstance(field['value'], bool) or not isinstance(field['value'], (int, float)) or not math.isfinite(field['value']):
                        row_errors.append('Invalid computed scalar: '+key)
                    else:
                        scalar_values[key].append(field['value'])
                else:
                    row_errors.append('Unrecognized feature state: '+key)
            if (known, unknown) != (result['known_features'], result['unknown_features']):
                row_errors.append('Envelope feature counts disagree')
            row['known_features'] = known
            row['unknown_features'] = unknown
            row['feature_record_sha256'] = feature_record['record_sha256']
            row['stage_resolution'] = result.get('stage_resolution')
            for stage in ['warp', 'composite']:
                stage_counts[stage+':'+result['stage_resolution'][stage]['kind']] += 1
        errors.extend(name+': '+e for e in row_errors)
        records.append(row)
    batches = []
    for path in sorted(BASE.glob('batch-*.zip')):
        with zipfile.ZipFile(path) as z:
            archive_manifest = json.loads(z.read('batch-manifest.json'))
            members_match = archive_manifest['run_identity'] == run_id and z.testzip() is None
            for row in archive_manifest['results']:
                name = row['preset']['relative_path']
                members_match &= hashlib.sha256(z.read(row['result_file'])).hexdigest() == row['result_sha256']
                members_match &= hashlib.sha256(z.read('presets/'+name)).hexdigest() == inventory[name]['sha256']
            batches.append({'file': str(path), 'sha256': file_digest(path),
                            'count': archive_manifest['completed_count'], 'partial': archive_manifest['partial'],
                            'crc_and_paired_member_hashes_pass': bool(members_match)})
            if not members_match:
                errors.append('Sealed batch verification failed: '+path.name)
    report = {'inspection_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
              'source_directory': str(BASE), 'run_identity': run_id,
              'run_manifest_sha256': hashlib.sha256(raw_manifest).hexdigest(),
              'inventory_count': len(inventory), 'batch_size': manifest['batch_size'],
              'snapshot_completed_files': len(records), 'status_counts': dict(status_counts),
              'simulation': configuration['simulation'], 'input_policy': configuration['signal_policy'],
              'feature_key_sets': [list(k) for k in sorted(keys_seen)],
              'unknown_feature_counts': dict(unknown_counts), 'unknown_reasons': dict(unknown_reasons),
              'stage_kinds': dict(stage_counts), 'producer_modules': module_results,
              'prepared_input_files_checked': len(inputs['files']),
              'schema_file': str(schema_path), 'schema_sha256': file_digest(schema_path),
              'frozen100_overlap_completed': matched_frozen,
              'frozen100_overlap_full_inventory': len({p['sha256'] for p in inventory.values()} & frozen_sources),
              'sealed_batches': batches, 'records': records, 'errors': errors,
              'scalar_ranges': {k: {'observations': len(v), 'minimum': min(v), 'maximum': max(v)} for k, v in scalar_values.items()},
              'limitations': ['Completed-file snapshot only; workers continue and later files are excluded.',
                              'Source schemas/digests checked; no predictor execution, source modification or native render comparison.',
                              'Parsed-source/compatibility/random/material payloads are not fully serialized here, so their declared digest strings are retained, not independently reconstructed.',
                              '47-field evidence is not an executable scene or proven reusable audio-response graph.']}
    (OUTPUT/'inspection.json').write_text(json.dumps(report, indent=2))
    print(json.dumps({'completed_snapshot': len(records), 'statuses': dict(status_counts),
                      'errors': errors, 'unknowns': dict(unknown_counts), 'stages': dict(stage_counts),
                      'frozen100_completed_overlap': len(matched_frozen)}, indent=2), flush=True)
    if errors:
        raise SystemExit(1)


if __name__ == '__main__':
    main()
