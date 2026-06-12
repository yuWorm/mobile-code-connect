<script setup lang="ts">
import 'vue-sonner/style.css'
import { Toaster } from 'vue-sonner'
</script>

<template>
  <Toaster rich-colors position="top-right" />
  <RouterView />
</template>
