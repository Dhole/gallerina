export const FileType = {
  Folder: 1,
  Image:  0,
};

// Configuration
export const serverUrl = "http://127.0.0.1:8080/api";
// export const serverUrl = "api";
// export const serverUrl = "/api";
export const defaultPlaySecs = 2;



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
