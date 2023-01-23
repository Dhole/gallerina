#include "sqlite3ext.h"
SQLITE_EXTENSION_INIT1
#include <assert.h>

#include "wyhash.h"

/* Hash input using wyhash */
static void hash_func(
  sqlite3_context *context,
  int argc,
  sqlite3_value **argv
){
  const unsigned char *zIn;
  int nIn;
  uint64_t zOut;
  assert( argc==1 );
  if( sqlite3_value_type(argv[0])==SQLITE_NULL ) return;
  zIn = (const unsigned char*)sqlite3_value_text(argv[0]);
  nIn = sqlite3_value_bytes(argv[0]);
  zOut = wyhash(zIn, nIn, 0, _wyp);
  sqlite3_result_int64(context, zOut);
}

#ifdef _WIN32
__declspec(dllexport)
#endif
int sqlite3_hash_init(
  sqlite3 *db,
  char **pzErrMsg,
  const sqlite3_api_routines *pApi
){
  int rc = SQLITE_OK;
  SQLITE_EXTENSION_INIT2(pApi);
  (void)pzErrMsg;  /* Unused parameter */
  rc = sqlite3_create_function(db, "hash", 1,
                   SQLITE_UTF8|SQLITE_INNOCUOUS|SQLITE_DETERMINISTIC,
                   0, hash_func, 0, 0);
  return rc;
}

