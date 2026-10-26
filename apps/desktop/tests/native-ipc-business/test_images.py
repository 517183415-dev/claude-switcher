"""Pure image receipt/bytes regression oracles; no native launch."""
import base64
import copy
from pathlib import Path
import unittest
import hashlib
import tempfile
import subprocess
import json
from test_backup_assets import png
import images

class ImageOracleTests(unittest.TestCase):
    def test_embedded_sources_work_when_csp_denies_fetch(self):
        # Model the actual connect-src policy: all fetch calls fail. Static
        # module imports remain permitted by script-src self.
        script = """
import {runImages} from './images.mjs';
globalThis.fetch=async()=>{throw new TypeError('Load failed')};
const captured=[];
async function invoke(command,args={}) {
 if(command==='ad_draft_read')return {};
 if(command==='ipc_business_draft_snapshot')return {};
 if(command==='ad_image_preview'){if(args.sha256==='f'.repeat(64))throw Error('missing');return 'preview';}
 if(command==='import_ad_image'){
  captured.push(args.bytes);
  if(captured.length>3)throw Error('rejected');
  return {asset:{sha256:'a'},preview_data_url:'preview'};
 }
 throw Error('unexpected command');
}
await runImages(invoke,'personal',{});
console.log(JSON.stringify(captured.slice(0,2)));
"""
        result=subprocess.run(['/opt/homebrew/bin/node','--input-type=module','-e',script],cwd=Path(__file__).parent,capture_output=True,text=True,timeout=20)
        self.assertEqual(result.returncode,0,result.stderr)
        data=json.loads(result.stdout)
        for actual,name in zip(data,('backup-source.png','image-source.jpg'),strict=True):
            self.assertEqual(bytes(actual),(Path(__file__).parent/name).read_bytes())

    def test_native_imageio_decodes_synthetic_jpeg_pixel(self):
        import jpeg_oracle
        pixel=jpeg_oracle.decode((Path(__file__).parent/'image-source.jpg').read_bytes())
        self.assertTrue(all(abs(a-b)<=3 for a,b in zip(pixel,[17,34,51,255],strict=True)),pixel)

    def test_native_imageio_rejects_corrupt_jpeg(self):
        import jpeg_oracle
        with self.assertRaises(ValueError):jpeg_oracle.decode(b'\xff\xd8\xff')

    def rows(self):
        initial={'synthetic':'unchanged draft'}
        jpeg=(Path(__file__).parent/'image-source.jpg').read_bytes()
        offset=2;clean=bytearray(jpeg[:2])
        while offset<len(jpeg)-2:
            marker=jpeg[offset+1];length=int.from_bytes(jpeg[offset+2:offset+4],'big')
            if marker==0xda:clean.extend(jpeg[offset:]);break
            if marker not in (0xfe,0xe1,0xe2):clean.extend(jpeg[offset:offset+2+length])
            offset+=2+length
        bodies=[png([17,34,51,255]),bytes(clean)]
        sizes=[(Path(__file__).parent/'backup-source.png').stat().st_size,len(jpeg),1048576,8,3,6,1048577,0]
        rows=[];files={'active.json':'a'*64}
        for i,name in enumerate(images.CASES):
            before={'label':name+':before','files':files.copy()}
            value=preview=None
            if i<3:
                data=bodies[1 if i==1 else 0];mime='image/jpeg' if i==1 else 'image/png'
                asset=dict(sha256=hashlib.sha256(data).hexdigest(),mime=mime,bytes=len(data),width=1,height=1)
                preview='data:'+mime+';base64,'+base64.b64encode(data).decode()
                value=dict(asset=asset,preview_data_url=preview)
                if i<2:files['state.json']=str(i+1)*64
            after={'label':name+':after','files':files.copy()}
            rows.append(dict(id=name,size=sizes[i],accepted=i<3,value=value,preview=preview,error=None if i<3 else images.ERRORS[name],before=before,after=after,draft=copy.deepcopy(initial)))
        return initial,rows
    def verify_rows(self,change=None):
        initial,rows=self.rows()
        if change:change(rows)
        events=[r[k] for r in rows for k in ('before','after')]
        return images.validate_rows(rows,events,initial,rows[0]['size'],rows[1]['size'])
    def test_accepts_png_jpeg_exact_limit_and_precise_rejection_matrix(self): self.assertEqual(len(self.verify_rows()[0]),2)
    def reject_rows(self,change):
        with self.assertRaises(AssertionError):self.verify_rows(change)
    def test_rejects_import_preview_differing_from_preview_command(self):self.reject_rows(lambda r:r[0].update(preview='data:image/png;base64,AA=='))
    def test_rejects_one_byte_over_limit_reported_as_success(self):self.reject_rows(lambda r:r[6].update(accepted=True))
    def test_rejects_corrupt_import_changing_state(self):self.reject_rows(lambda r:r[3]['after']['files'].update(extra='b'*64))
    def test_rejects_draft_mutation_after_import(self):self.reject_rows(lambda r:r[0].update(draft={}))
    def test_rejects_exact_limit_reimport_changing_assets(self):self.reject_rows(lambda r:r[2]['after']['files'].update(extra='b'*64))
    def test_rejects_missing_image_case(self):self.reject_rows(lambda r:r.pop())
    def test_rejects_wrong_normalized_byte_hash(self):self.reject_rows(lambda r:r[0]['value']['asset'].update(sha256='f'*64))

    def test_decodes_actual_fixture_jpeg_dimensions(self):
        data=(Path(__file__).parent/'image-source.jpg').read_bytes()
        self.assertEqual(images.jpeg_dimensions(data, normalized=False),(1,1))
    def test_rejects_metadata_in_normalized_jpeg(self):
        with self.assertRaises(AssertionError): images.jpeg_dimensions((Path(__file__).parent/'image-source.jpg').read_bytes())
    def test_rejects_truncated_jpeg(self):
        with self.assertRaises(AssertionError): images.jpeg_dimensions(b'\xff\xd8\xff')
    def test_rejects_data_url_with_wrong_mime(self):
        with self.assertRaises(AssertionError): images.preview_bytes('data:image/jpeg;base64,AA==','image/png')
    def test_rejects_invalid_preview_base64(self):
        with self.assertRaises(AssertionError): images.preview_bytes('data:image/png;base64,***','image/png')
    def test_public_accepts_exact_permission_denials(self): images.validate_public([{'command':c,'kind':'permission'} for c in images.COMMANDS])
    def test_public_rejects_missing_handler_as_acl_proof(self):
        with self.assertRaises(AssertionError): images.validate_public([{'command':c,'kind':'missing'} for c in images.COMMANDS])
    def test_rejects_wrong_image_error_category(self):
        with self.assertRaises(AssertionError): images.validate_error('corrupt_png','network unavailable')
    def test_accepts_exact_predecoder_size_rejection(self): images.validate_error('oversize','图片超过 1 MiB 上限')

if __name__=='__main__':unittest.main()
