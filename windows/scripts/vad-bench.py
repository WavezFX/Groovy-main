# Round 4B item 6: the energy detector against Silero VAD on 20 utterances made
# with the light offline voice (sherpa-onnx 1.13.8, Python), each with a pause of
# 0.25-1.2 s in it, every third ending softly, room noise added. Needs the light
# voice's files in $VAD_BENCH_MODELS/voice and silero_vad.onnx (MIT) beside them.
# Results: STATUS.md, round 4B.
import sys,time,os,resource
sys.path.insert(0,os.environ.get('SHERPA_PY','.'))
import numpy as np, sherpa_onnx as so
D=os.environ.get('VAD_BENCH_MODELS','models')
tts=so.OfflineTts(so.OfflineTtsConfig(model=so.OfflineTtsModelConfig(vits=so.OfflineTtsVitsModelConfig(model=f'{D}/voice/en_US-libritts_r-medium.onnx',tokens=f'{D}/voice/tokens.txt',data_dir=f'{D}/voice/espeak-ng-data'),num_threads=2)))
SR=16000
def say(t,sid):
    a=tts.generate(t,sid=sid,speed=1.0); x=np.array(a.samples,dtype=np.float32)
    n=int(len(x)*SR/a.sample_rate); return np.interp(np.linspace(0,len(x)-1,n),np.arange(len(x)),x).astype(np.float32)
pairs=[("Hey Groovy","play some music"),("set a timer for","five minutes"),("what's the weather","in London tomorrow"),("open","my downloads folder"),("remind me to","call my brother at six"),("Yo Groovy","turn the volume down"),("can you","read my last email"),("how long until","the next meeting"),("skip","this song"),("add milk","to the shopping list"),("Groovy","what time is it"),("send a message to","Sam saying I'm late"),("pause","the music please"),("what's on","my calendar today"),("turn on","game mode"),("translate","good morning into Spanish"),("look at","my screen"),("start","a five minute break"),("search the web for","cheap flights to Madrid"),("tell me","a short joke")]
rng=np.random.default_rng(7)
utts=[]
for i,(a,b) in enumerate(pairs):
    pause=0.25+0.05*i            # 250 ms .. 1.2 s
    quiet=0.35 if i%3==0 else 1.0 # every third ends softly
    x=np.concatenate([np.zeros(int(0.5*SR)),say(a,i*7%900),np.zeros(int(pause*SR)),quiet*say(b,i*7%900),np.zeros(int(2.0*SR))]).astype(np.float32)
    x+=rng.normal(0,0.003,len(x)).astype(np.float32)  # room noise
    speech_end=len(x)-int(2.0*SR)
    utts.append((x,speech_end,pause))
def energy(x,hang=25,adapt=False):
    F=480;floor=0.004;ins=False;sf=0;si=0;cur=0;start=0;segs=[]
    for k in range(0,len(x)-F+1,F):
        fr=x[k:k+F];rms=float(np.sqrt(np.mean(fr*fr)));sp=rms>max(floor*3,0.012)
        if not sp and not ins: floor=floor*0.97+rms*0.03
        if ins:
            si=0 if sp else si+1
            cur+=1
            h=hang+12 if adapt and cur<60 else hang
            if si>=h: segs.append(k+F);ins=False;sf=0;si=0
        elif sp:
            sf+=1
            if sf>=3: ins=True;cur=0
        else: sf=0
    return segs
def silero(x,minsil=0.75):
    c=so.VadModelConfig();c.silero_vad.model=f'{D}/silero_vad.onnx';c.silero_vad.min_silence_duration=minsil;c.silero_vad.min_speech_duration=0.1;c.silero_vad.threshold=0.5;c.sample_rate=SR;c.num_threads=1
    v=so.VoiceActivityDetector(c,buffer_size_in_seconds=30);segs=[]
    W=512
    for k in range(0,len(x)-W+1,W):
        v.accept_waveform(x[k:k+W])
        while not v.empty():
            s=v.front; segs.append((s.start+len(s.samples), k+W)); v.pop()
    return segs
def score(fn):
    early=0;lat=[];t0=time.process_time();secs=0
    for x,end,_ in utts:
        secs+=len(x)/SR
        s=fn(x)
        pairs_=[e if isinstance(e,tuple) else (e,e) for e in s]
        ends=[a for a,_ in pairs_]; emit={a:b for a,b in pairs_}
        if not ends or ends[0] < end-int(0.05*SR) and len(ends)>1 or (ends and ends[0]<end-int(0.05*SR)): early+=1
        last=[e for e in ends if e>=end-int(0.05*SR)]
        if last: lat.append((emit[last[0]]-end)/SR*1000)
    cpu=(time.process_time()-t0)/secs*100
    return early,(np.median(lat) if lat else float('nan')),cpu
r0=resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
for name,fn in [("energy, 750 ms",lambda x:energy(x,25)),("energy, 750 ms, 1110 ms in the first 1.8 s",lambda x:energy(x,25,True)),("energy, 1050 ms",lambda x:energy(x,35)),("Silero, 750 ms",lambda x:silero(x,0.75)),("Silero, 500 ms",lambda x:silero(x,0.5)),("Silero, 1050 ms",lambda x:silero(x,1.05))]:
    e,l,c=score(fn); print(f"| {name} | {e} of 20 | {l:.0f} ms | {c:.2f} % of one core |")
print("rss MB",resource.getrusage(resource.RUSAGE_SELF).ru_maxrss/1024);print("pauses", [round(p,2) for _,_,p in utts])
