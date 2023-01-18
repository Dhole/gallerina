export const FileType = {
  Folder: 1,
  Image:  0,
};

// Configuration
// export const serverUrl = "http://127.0.0.1:8080/api"; // local testing
export const serverUrl = "api"; // Production
// export const serverUrl = "/api";
export const defaultPlaySecs = 5;

export const emptyCfg = {
  sort: "name",
  reverse: false,
  raw: false,
  recursive: false,
  randSeed: 0,
}


export function apiUrl(path, params) {
  if (params.dir === "") {
    params.dir = "/";
  }
  let p = new URLSearchParams(params);
  let u = `${serverUrl}/${path}?${p.toString()}`;
  return u;
}

export function uiUrl(params) {
  let p = new URLSearchParams(params);
  let u = `?${p.toString()}`;
  return u;
}


export function cfg2str(cfg) {
  return `${cfg.sort}-${cfg.reverse ? 1 : 0}-${cfg.raw ? 1 : 0}-${cfg.recursive ? 1 : 0}-${cfg.randSeed}`;
}

export function str2cfg(str) {
  let values = str.split("-");
  let randSeed = parseInt(values[4], 10);
  return {
    sort: values[0],
    reverse: values[1] === "0" ? false : true,
    raw: values[2] === "0" ? false : true,
    recursive: values[3] === "0" ? false : true,
    randSeed: isNaN(randSeed) ? 0 : randSeed,
  };
}

export function trimPrefix(str, prefix) {
  if (str.startsWith(prefix)) {
    return str.slice(prefix.length)
  } else {
    return str
  }
}

// 1993 Park-Miller LCG
function LCG(s) {
  s = s+1;
  return function() {
    s = Math.imul(48271, s) | 0 % 2147483647;
    return (s & 2147483647) / 2147483648;
  }
}

export function shuffleArray(array, seed) {
  let random = LCG(seed);
  for (let i = array.length - 1; i > 0; i--) {
    const j = Math.floor(random() * (i + 1));
    [array[i], array[j]] = [array[j], array[i]];
  }
}

export function getExt(filename) {
  let parts = filename.split(".");
  if (parts.length === 0) {
    return "";
  }
  return parts[parts.length-1].toLowerCase();
}

// Return true if it's image, false if it's video
export function isImg(filename) {
  let ext = getExt(filename);
  if (ext === "mp4" || ext === "mov") {
    return false;
  } else {
    return true;
  }
}
