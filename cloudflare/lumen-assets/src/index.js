export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    const key = decodeURIComponent(url.pathname.replace(/^\/+/, ""));
    if (!key || key.includes("..")) {
      return new Response("Chemin invalide", { status: 400 });
    }

    if (request.method === "PUT") {
      if (request.headers.get("X-Lumen-Upload") !== env.UPLOAD_KEY) {
        return new Response("Interdit", { status: 401 });
      }
      await env.BUCKET.put(key, request.body, {
        httpMetadata: {
          contentType: request.headers.get("content-type") || "application/octet-stream",
          cacheControl: "public, max-age=86400",
        },
      });
      return new Response("ok");
    }

    if (request.method === "GET" || request.method === "HEAD") {
      const object = await env.BUCKET.get(key);
      if (!object) return new Response("Introuvable", { status: 404 });
      const headers = new Headers();
      object.writeHttpMetadata(headers);
      headers.set("etag", object.httpEtag);
      headers.set("cache-control", "public, max-age=86400");
      return new Response(request.method === "HEAD" ? null : object.body, { headers });
    }

    return new Response("Méthode refusée", { status: 405 });
  },
};
